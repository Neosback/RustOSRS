//! Owned native process jobs. The UI never waits for Python, the Lab server, or a worker join.
use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
};

pub(super) struct Job {
    child: Option<Child>,
    events: Receiver<String>,
    cancel_file: PathBuf,
    pub(super) cancelled: bool,
}
impl Job {
    pub(super) fn start(
        python: &Path,
        script: &Path,
        args: &[String],
        directory: &Path,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let cancel_file = directory.join("cancel-requested");
        if cancel_file.exists() {
            std::fs::remove_file(&cancel_file).map_err(|e| e.to_string())?;
        }
        let stderr =
            std::fs::File::create(directory.join("worker.log")).map_err(|e| e.to_string())?;
        let mut command = Command::new(python);
        command
            .arg(script)
            .args(args)
            .arg("--cancel-file")
            .arg(&cancel_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(stderr);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("Could not start policy worker: {e}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Policy worker has no progress stream")?;
        let (send, events) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let line: String = line.chars().take(8192).collect();
                if send.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child: Some(child),
            events,
            cancel_file,
            cancelled: false,
        })
    }
    pub(super) fn events(&self) -> Vec<String> {
        self.events.try_iter().take(64).collect()
    }
    pub(super) fn poll(&mut self) -> Option<Result<(), String>> {
        let child = self.child.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                self.child = None;
                Some(if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Policy worker exited with {status}; see worker.log"
                    ))
                })
            }
            Ok(None) => None,
            Err(e) => Some(Err(e.to_string())),
        }
    }
    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
        let _ = std::fs::write(&self.cancel_file, b"cancel");
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                for _ in 0..100 {
                    if child.try_wait().ok().flatten().is_some() {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
                #[cfg(unix)]
                {
                    let _ = Command::new("/bin/kill")
                        .args(["-TERM", "--", &format!("-{}", child.id())])
                        .status();
                }
                let _ = child.kill();
                let _ = child.wait();
            });
        }
    }
}
