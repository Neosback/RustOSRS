//! Safe lazy file access on native platforms; shared immutable bytes in WASM.

use std::{io, sync::Arc};

#[derive(Debug)]
pub(crate) enum Backing {
    #[cfg(any(unix, windows))]
    File(std::fs::File),
    #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
    File(std::sync::Mutex<std::fs::File>),
    Owned(Arc<Vec<u8>>),
}

impl Backing {
    fn owned(&self) -> Option<&[u8]> {
        match self {
            Self::Owned(bytes) => Some(bytes),
            #[cfg(not(target_arch = "wasm32"))]
            Self::File(_) => None,
        }
    }

    fn read_at(&self, offset: usize, output: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Owned(bytes) => {
                let available = bytes.get(offset..).ok_or_else(truncated)?;
                let count = available.len().min(output.len());
                output[..count].copy_from_slice(&available[..count]);
                Ok(count)
            }
            #[cfg(unix)]
            Self::File(file) => {
                use std::os::unix::fs::FileExt;
                read_until_eof(output, |buffer, done| {
                    file.read_at(buffer, (offset + done) as u64)
                })
            }
            #[cfg(windows)]
            Self::File(file) => {
                use std::os::windows::fs::FileExt;
                read_until_eof(output, |buffer, done| {
                    file.seek_read(buffer, (offset + done) as u64)
                })
            }
            #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
            Self::File(file) => {
                use std::io::{Read, Seek, SeekFrom};
                let mut reader = file
                    .lock()
                    .map_err(|_| io::Error::other("cache reader lock poisoned"))?;
                reader.seek(SeekFrom::Start(offset as u64))?;
                read_until_eof(output, |buffer, _| reader.read(buffer))
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_until_eof(
    output: &mut [u8],
    mut read: impl FnMut(&mut [u8], usize) -> io::Result<usize>,
) -> io::Result<usize> {
    let mut done = 0;
    while done < output.len() {
        match read(&mut output[done..], done) {
            Ok(0) => break,
            Ok(count) => done += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(done)
}

/// Per-group read-ahead amortizes native syscalls for adjacent sectors. Every
/// sector is validated and followed by its actual next pointer. A fragmented
/// chain refills; nothing is retained between groups or sources.
pub(crate) struct SectorReader<'a> {
    source: &'a Backing,
    bytes: [u8; crate::SECTOR_SIZE * 16],
    start: usize,
    valid: usize,
}

impl<'a> SectorReader<'a> {
    pub(crate) fn new(source: &'a Backing) -> Self {
        Self {
            source,
            bytes: [0; crate::SECTOR_SIZE * 16],
            start: 0,
            valid: 0,
        }
    }

    pub(crate) fn sector(
        &mut self,
        offset: usize,
        len: usize,
        remaining: usize,
    ) -> io::Result<&[u8]> {
        let end = offset.checked_add(len).ok_or_else(truncated)?;
        if let Some(bytes) = self.source.owned() {
            return bytes.get(offset..end).ok_or_else(truncated);
        }
        if offset < self.start || end > self.start + self.valid {
            let count = remaining.min(self.bytes.len());
            offset.checked_add(count).ok_or_else(truncated)?;
            self.valid = self.source.read_at(offset, &mut self.bytes[..count])?;
            self.start = offset;
        }
        let start = offset.checked_sub(self.start).ok_or_else(truncated)?;
        self.bytes[..self.valid]
            .get(start..start + len)
            .ok_or_else(truncated)
    }
}

fn truncated() -> io::Error {
    io::Error::new(io::ErrorKind::UnexpectedEof, "cache sector is truncated")
}
