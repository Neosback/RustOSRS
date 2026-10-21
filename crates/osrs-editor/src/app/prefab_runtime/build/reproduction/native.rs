//! Native filesystem boundary. Headless verification never initializes an app, GPU or cache reader.
use super::*;
use std::{
    io::Read,
    path::Path,
    sync::mpsc::{self, Receiver, TryRecvError},
};
pub(in super::super) struct Export {
    receive: Receiver<Result<String, String>>,
}
impl Export {
    pub(in super::super) fn start(capture: Capture, path: std::path::PathBuf) -> Self {
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = capture
                .encode()
                .map_err(|e| e.to_string())
                .and_then(|text| {
                    crate::export_flow::publish_new_file(&path, text.as_bytes())?;
                    Ok(path.display().to_string())
                });
            let _ = send.try_send(result);
        });
        Self { receive }
    }
    pub(in super::super) fn poll(&self) -> Option<Result<String, String>> {
        match self.receive.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(_) => Some(Err("Reproduction export stopped".into())),
        }
    }
}
pub(crate) fn run_cli(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        let error = Error::Input("Usage: osrs-editor --reproduce-building BUNDLE.json".into());
        println!("{}", serde_json::json!({"status":"FAILED","error":error}));
        return Err(error.to_string());
    }
    let path = Path::new(&args[0]);
    let result = (|| {
        let metadata = std::fs::metadata(path)
            .map_err(|e| Error::Input(format!("{}: {e}", path.display())))?;
        if !metadata.is_file() {
            return Err(Error::Input(
                "A reproduction bundle must be a regular file".into(),
            ));
        }
        if metadata.len() > MAX_BYTES as u64 {
            return Err(Error::Input("Bundle exceeds 128 MiB".into()));
        }
        let file = std::fs::File::open(path)
            .map_err(|e| Error::Input(format!("{}: {e}", path.display())))?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| Error::Input(e.to_string()))?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::Input("Bundle exceeds 128 MiB".into()));
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| Error::Schema(e.to_string()))?;
        verify_any(text)
    })();
    match result {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Err(error) => {
            println!("{}", serde_json::json!({"status":"FAILED","error":error}));
            Err(error.to_string())
        }
    }
}
pub(in super::super) use Export as ExportTask;
