//! Safe lazy file access on native platforms; shared immutable bytes in WASM.

use std::{io, sync::Arc};

#[derive(Debug)]
pub(crate) enum Backing {
    #[cfg(not(target_arch = "wasm32"))]
    File(std::fs::File),
    Owned(Arc<Vec<u8>>),
}

impl Backing {
    pub(crate) fn read_exact_at(&self, offset: usize, output: &mut [u8]) -> io::Result<()> {
        match self {
            Self::Owned(bytes) => {
                let end = offset.checked_add(output.len()).ok_or_else(truncated)?;
                let data = bytes.get(offset..end).ok_or_else(truncated)?;
                output.copy_from_slice(data);
                Ok(())
            }
            #[cfg(unix)]
            Self::File(file) => {
                use std::os::unix::fs::FileExt;
                file.read_exact_at(output, offset as u64)
            }
            #[cfg(windows)]
            Self::File(file) => {
                use std::os::windows::fs::FileExt;
                let mut remaining = output;
                let mut position = offset as u64;
                while !remaining.is_empty() {
                    match file.seek_read(remaining, position) {
                        Ok(0) => return Err(truncated()),
                        Ok(count) => {
                            position += count as u64;
                            remaining = &mut remaining[count..];
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            }
            #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
            Self::File(file) => {
                use std::io::{Read, Seek, SeekFrom};
                let mut reader = file.try_clone()?;
                reader.seek(SeekFrom::Start(offset as u64))?;
                reader.read_exact(output)
            }
        }
    }
}

fn truncated() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "cache sector is truncated")
}
