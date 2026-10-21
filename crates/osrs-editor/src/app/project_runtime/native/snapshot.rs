//! One explicit immutable authored-content export, separate from project-save lifecycle state.
use super::*;
use std::sync::mpsc::{self, TryRecvError};

pub(in crate::app::project_runtime) struct SnapshotJob {
    receive: Receiver<Result<PathBuf, String>>,
}
impl ProjectRuntime {
    pub(in crate::app) fn authored_snapshot_running(&self) -> bool {
        self.snapshot.is_some()
    }

    pub(in crate::app) fn start_authored_snapshot(
        &mut self,
        path: PathBuf,
        session: &EditRuntime,
    ) -> Result<(), String> {
        if self.authored_snapshot_running() {
            return Err("An authored snapshot export is already running".into());
        }
        let frozen = session.freeze_authored_snapshot();
        let (send, receive) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = frozen.encode().map_err(|e| e.to_string()).and_then(|text| {
                crate::export_flow::publish_new_file(&path, text.as_bytes())?;
                Ok(path)
            });
            let _ = send.try_send(result);
        });
        self.snapshot = Some(SnapshotJob { receive });
        Ok(())
    }

    pub(in crate::app) fn poll_authored_snapshot(&mut self) -> Option<Result<PathBuf, String>> {
        let result = match self.snapshot.as_ref()?.receive.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => {
                Err("Authored snapshot export stopped unexpectedly".into())
            }
        };
        self.snapshot = None;
        Some(result)
    }
}

#[cfg(test)]
mod tests;
