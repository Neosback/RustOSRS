//! Low-level JS5 sector chains, index metadata and container compression.
//!
//! Derived from rune-fs 0.2.0 (MIT); see LICENSE and README.md. This crate owns
//! storage mechanics. Cache build profiles, semantic codecs, source identity,
//! logical-file limits and verified export remain in `osrs-cache`.

mod archive;
pub mod codec;
pub mod error;
mod index;
pub mod parse;
mod sector;
mod source;
pub mod xtea;

#[doc(inline)]
pub use error::Error;
use error::Result;

pub const MAIN_DATA: &str = "main_file_cache.dat2";
pub const REFERENCE_TABLE: &str = "main_file_cache.idx255";
pub const REFERENCE_TABLE_ID: u8 = 255;

pub use archive::*;
pub use index::*;
pub use sector::*;

use crate::codec::{Buffer, Encoded};
use error::ParseError;
use source::Backing;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// A virtual file type for the `.dat2` file.
#[derive(Debug)]
pub struct Dat2(Backing);

impl Dat2 {
    /// Open the `.dat2` file without loading it into process-owned memory.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let file = std::fs::File::open(path)?;
        #[cfg(not(any(unix, windows)))]
        let file = std::sync::Mutex::new(file);
        Ok(Self(Backing::File(file)))
    }

    /// Browser callers supply bytes through [`Dat2::from_bytes`].
    #[cfg(target_arch = "wasm32")]
    pub fn new<P: AsRef<Path>>(_path: P) -> crate::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "native cache files are unavailable in the browser; use Dat2::from_bytes",
        )
        .into())
    }

    /// A `.dat2` that is already in memory.
    pub fn from_bytes(bytes: Arc<Vec<u8>>) -> Self {
        Self(Backing::Owned(bytes))
    }

    /// Read all the data that belongs to the `ArchiveRef`.
    pub fn read(&self, archive_ref: &ArchiveRef) -> crate::Result<Buffer<Encoded>> {
        if archive_ref.length > 0x00ff_ffff {
            return Err(ParseError::Archive(archive_ref.id).into());
        }
        let mut buffer = Buffer::from(Vec::with_capacity(archive_ref.length));
        self.read_into_writer(archive_ref, &mut buffer)?;

        Ok(buffer)
    }

    /// Read all the data that belongs to the `ArchiveRef` into the given writer.
    pub fn read_into_writer<W>(&self, archive_ref: &ArchiveRef, writer: &mut W) -> crate::Result<()>
    where
        W: Write,
    {
        if archive_ref.length > 0x00ff_ffff {
            return Err(ParseError::Archive(archive_ref.id).into());
        }
        let mut current = archive_ref.sector;
        let header_size = SectorHeaderSize::from(archive_ref);
        let mut reader = source::SectorReader::new(&self.0);
        let mut remaining: usize = archive_ref.data_blocks().sum();

        for (chunk, data_len) in archive_ref.data_blocks().enumerate() {
            let offset = current
                .checked_mul(SECTOR_SIZE)
                .filter(|_| current != 0)
                .ok_or(ParseError::Sector(current))?;
            let data_block = reader.sector(offset, data_len, remaining)?;
            match Sector::new(data_block, &header_size) {
                Ok(sector) => {
                    sector
                        .header
                        .validate(archive_ref.id, chunk, archive_ref.index_id)?;
                    current = sector.header.next;
                    writer.write_all(sector.data_block)?;
                    remaining -= data_len;
                }
                Err(_) => return Err(ParseError::Sector(archive_ref.sector).into()),
            };
        }

        Ok(())
    }

    pub fn metadata(&self, archive_ref: &ArchiveRef) -> crate::Result<IndexMetadata> {
        let buffer = self.read(archive_ref)?.decode()?;
        IndexMetadata::from_buffer(buffer)
    }
}

#[cfg(test)]
fn is_normal<T: Send + Sync + Sized + Unpin>() {}
#[test]
fn normal_types() {
    is_normal::<Dat2>();
}
