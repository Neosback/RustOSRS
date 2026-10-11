//! Read-only, low level, virtual file types for the RuneScape file system.
//! 
//! This crate supplies all of the backing types for [rs-cache](https://docs.rs/rs-cache). Many of these
//! types were private but are now publicly available. rs-cache is a high level api for both the OSRS and RS3 
//! caches and exposing these low level virtual types didn't make sense, hence this crate.
//! 
//! A word of caution, these types are highly experimental, I have done my best to document and test as
//! much as I can, but there might still be the weird occasional edge-case. With that said, whenever you find
//! a bug or missing feature; or even unsoundness don't hesitate to 
//! [open an issue](https://github.com/jimvdl/rs-cache/issues/new).

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(
    clippy::all,
    clippy::correctness,
    clippy::suspicious,
    clippy::style,
    clippy::complexity,
    clippy::perf
)]

mod archive;
pub mod codec;
pub mod error;
mod index;
pub mod parse;
mod sector;
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
#[cfg(not(target_arch = "wasm32"))]
use memmap2::Mmap;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

/// Where the bytes of the `.dat2` file live.
#[derive(Debug)]
enum Backing {
    /// A memory map over the file (native builds).
    #[cfg(not(target_arch = "wasm32"))]
    Mapped(Mmap),
    /// The whole file in memory (the browser build, where the page reads it through the File API).
    Owned(Arc<Vec<u8>>),
}

/// A virtual file type for the `.dat2` file.
#[derive(Debug)]
pub struct Dat2(Backing);

impl Dat2 {
    /// Initializes a memory map over the specified `.dat2` file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        Ok(Self(Backing::Mapped(unsafe {
            Mmap::map(&std::fs::File::open(path.as_ref())?)?
        })))
    }

    /// There is no file system to map in the browser; use [`Dat2::from_bytes`].
    #[cfg(target_arch = "wasm32")]
    pub fn new<P: AsRef<Path>>(_path: P) -> crate::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "memory maps are not available in the browser; read the file into memory",
        )
        .into())
    }

    /// A `.dat2` that is already in memory.
    pub fn from_bytes(bytes: Arc<Vec<u8>>) -> Self {
        Self(Backing::Owned(bytes))
    }

    fn bytes(&self) -> &[u8] {
        match &self.0 {
            #[cfg(not(target_arch = "wasm32"))]
            Backing::Mapped(map) => map,
            Backing::Owned(bytes) => bytes,
        }
    }

    /// Read all the data that belongs to the `ArchiveRef`.
    pub fn read(&self, archive_ref: &ArchiveRef) -> crate::Result<Buffer<Encoded>> {
        let mut buffer = Buffer::from(Vec::with_capacity(archive_ref.length));
        self.read_into_writer(archive_ref, &mut buffer)?;

        assert_eq!(buffer.len(), archive_ref.length);

        Ok(buffer)
    }

    /// Read all the data that belongs to the `ArchiveRef` into the given writer.
    pub fn read_into_writer<W>(&self, archive_ref: &ArchiveRef, writer: &mut W) -> crate::Result<()>
    where
        W: Write,
    {
        let mut current = archive_ref.sector;
        let header_size = SectorHeaderSize::from(archive_ref);

        for (chunk, data_len) in archive_ref.data_blocks().enumerate() {
            let offset = current * SECTOR_SIZE;

            let data_block = self
                .bytes()
                .get(offset..offset + data_len)
                .ok_or(ParseError::Sector(archive_ref.sector))?;
            match Sector::new(data_block, &header_size) {
                Ok(sector) => {
                    sector
                        .header
                        .validate(archive_ref.id, chunk, archive_ref.index_id)?;
                    current = sector.header.next;
                    writer.write_all(sector.data_block)?;
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
