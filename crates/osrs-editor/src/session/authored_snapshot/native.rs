//! Bounded read-only native checker. No app, source/cache, GPU or live session is initialized.
use super::*;
use std::{io::Read, path::Path};
pub(crate) fn run_cli(args: &[String]) -> Result<(), String> {
    let result = (|| {
        if args.len() != 1 {
            return Err("Usage: osrs-editor --verify-authored-snapshot FILE.json".into());
        }
        let path = Path::new(&args[0]);
        let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES as u64 {
            return Err("An authored snapshot must be a regular file of at most 128 MiB".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|e| e.to_string())?
            .take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("Authored snapshot exceeds 128 MiB".into());
        }
        let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        verify(text).map_err(|e| e.to_string())
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
            println!("{}", serde_json::json!({"status":"FAILED", "error":error}));
            Err(error)
        }
    }
}
