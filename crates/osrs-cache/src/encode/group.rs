use runefs::codec::{Buffer, Compression, Decoded, Encoded};
use std::fmt;

/// Container compression applied around a packed group (the compression byte of the JS5
/// container header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerCompression {
    None,
    Gzip,
    Bzip2,
}

impl From<ContainerCompression> for Compression {
    fn from(value: ContainerCompression) -> Self {
        match value {
            ContainerCompression::None => Self::None,
            ContainerCompression::Gzip => Self::Gzip,
            ContainerCompression::Bzip2 => Self::Bzip2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    /// A file in the group is larger than the format's signed 32-bit size deltas allow.
    FileTooLarge { index: usize },
    /// The transport layer could not compress or decompress the container.
    Codec { detail: String },
}

impl fmt::Display for ContainerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FileTooLarge { index } => write!(formatter, "group file {index} is too large"),
            Self::Codec { detail } => write!(formatter, "container codec: {detail}"),
        }
    }
}

impl std::error::Error for ContainerError {}

/// Pack logical files into one group payload using a single chunk: the files back to back, then
/// one big-endian `i32` size delta per file, then the chunk count (`1`). A one-file group is the
/// file itself, matching how groups are split on read.
pub fn pack_group(files: &[&[u8]]) -> Result<Vec<u8>, ContainerError> {
    if let [only] = files {
        return Ok(only.to_vec());
    }
    let data_len: usize = files.iter().map(|file| file.len()).sum();
    let mut out = Vec::with_capacity(data_len + files.len() * 4 + 1);
    for file in files {
        out.extend_from_slice(file);
    }
    let mut previous = 0i64;
    for (index, file) in files.iter().enumerate() {
        let size = i64::try_from(file.len()).map_err(|_| ContainerError::FileTooLarge { index })?;
        let delta =
            i32::try_from(size - previous).map_err(|_| ContainerError::FileTooLarge { index })?;
        out.extend_from_slice(&delta.to_be_bytes());
        previous = size;
    }
    out.push(1);
    Ok(out)
}

/// Split a payload produced by [`pack_group`] (or read from a cache) into its `file_count` files,
/// numbered `0..file_count`.
pub fn split_group(
    payload: &[u8],
    file_count: usize,
) -> Result<Vec<(u32, Vec<u8>)>, ContainerError> {
    let ids: Vec<u32> = (0..file_count as u32).collect();
    crate::transport::split_group_files(payload, &ids, file_count.max(1)).map_err(|error| {
        ContainerError::Codec {
            detail: format!("{error:?}"),
        }
    })
}

/// Wrap a group payload in a JS5 container (compression byte, lengths, data, optional version).
pub fn encode_container(
    payload: Vec<u8>,
    compression: ContainerCompression,
    version: Option<i16>,
) -> Result<Vec<u8>, ContainerError> {
    let mut buffer: Buffer<Decoded> = Buffer::from(payload).with_compression(compression.into());
    if let Some(version) = version {
        buffer = buffer.with_version(version);
    }
    buffer
        .encode()
        .map(Buffer::finalize)
        .map_err(|error| ContainerError::Codec {
            detail: error.to_string(),
        })
}

/// Unwrap a JS5 container produced by [`encode_container`] (or read from a cache).
pub fn decode_container(container: Vec<u8>) -> Result<Vec<u8>, ContainerError> {
    let buffer: Buffer<Encoded> = Buffer::from(container);
    buffer
        .decode()
        .map(Buffer::finalize)
        .map_err(|error| ContainerError::Codec {
            detail: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::split_group_files;

    #[test]
    fn packed_files_split_back_to_themselves() {
        let a = vec![1u8, 2, 3, 4, 5];
        let b = vec![9u8; 300];
        let packed = pack_group(&[&a, &b]).unwrap_or_else(|e| unreachable!("{e}"));
        let split = split_group_files(&packed, &[0, 1], 16)
            .unwrap_or_else(|e| unreachable!("valid group: {e:?}"));
        assert_eq!(split, vec![(0, a), (1, b)]);
    }

    #[test]
    fn a_one_file_group_is_the_file_itself() {
        assert_eq!(pack_group(&[&[7, 8, 9]]), Ok(vec![7, 8, 9]));
    }

    #[test]
    fn containers_round_trip_for_every_compression() {
        let payload: Vec<u8> = (0..4000u32).map(|i| (i % 251) as u8).collect();
        for compression in [
            ContainerCompression::None,
            ContainerCompression::Gzip,
            ContainerCompression::Bzip2,
        ] {
            let encoded = encode_container(payload.clone(), compression, Some(3))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let decoded = decode_container(encoded).unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(decoded, payload, "{compression:?}");
        }
    }
}
