use osrs_filestore::{ArchiveRef, Dat2, Index, SECTOR_SIZE};
use std::sync::Arc;

fn archive(id: u32) -> (ArchiveRef, Vec<u8>, Vec<u8>) {
    archive_len(id, 1111)
}

fn archive_len(id: u32, length: usize) -> (ArchiveRef, Vec<u8>, Vec<u8>) {
    let reference = ArchiveRef {
        id,
        index_id: 5,
        sector: 1,
        length,
    };
    let payload: Vec<_> = (0..reference.length).map(|i| (i % 251) as u8).collect();
    let header = if id > u16::MAX.into() { 10 } else { 8 };
    let mut bytes = vec![0; SECTOR_SIZE];
    let count = payload.len().div_ceil(SECTOR_SIZE - header);
    for (chunk, data) in payload.chunks(SECTOR_SIZE - header).enumerate() {
        if header == 10 {
            bytes.extend_from_slice(&id.to_be_bytes());
        } else {
            bytes.extend_from_slice(&(id as u16).to_be_bytes());
        }
        bytes.extend_from_slice(&(chunk as u16).to_be_bytes());
        let next = if chunk + 1 == count { 0 } else { chunk + 2 } as u32;
        bytes.extend_from_slice(&next.to_be_bytes()[1..]);
        bytes.push(5);
        bytes.extend_from_slice(data);
    }
    (reference, bytes, payload)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_fragmented_chain_crosses_read_ahead_windows_in_both_directions()
-> Result<(), Box<dyn std::error::Error>> {
    let (reference, contiguous, payload) = archive_len(70_000, 25_000);
    let chunks = reference.data_blocks().count();
    let mut order = Vec::new();
    for low in 1..=chunks.div_ceil(2) {
        order.push(low);
        let high = chunks + 1 - low;
        if high != low {
            order.push(high);
        }
    }
    let mut fragmented = vec![0; (chunks + 1) * SECTOR_SIZE];
    for (chunk, len) in reference.data_blocks().enumerate() {
        let start = order[chunk] * SECTOR_SIZE;
        fragmented[start..start + len].copy_from_slice(
            &contiguous[(chunk + 1) * SECTOR_SIZE..(chunk + 1) * SECTOR_SIZE + len],
        );
        let next = order.get(chunk + 1).copied().unwrap_or(0) as u32;
        fragmented[start + 6..start + 9].copy_from_slice(&next.to_be_bytes()[1..]);
    }
    let path = std::env::temp_dir().join(format!(
        "rustosrs-filestore-fragmented-{}.dat2",
        std::process::id()
    ));
    std::fs::write(&path, fragmented)?;
    let result = Dat2::new(&path)?.read(&reference)?;
    let _ = std::fs::remove_file(path);
    assert_eq!(result.as_slice(), payload);
    Ok(())
}

#[test]
fn owned_reads_normal_and_expanded_headers_with_unpadded_final_sector()
-> Result<(), osrs_filestore::Error> {
    for id in [1, 70_000] {
        let (reference, bytes, payload) = archive(id);
        let data = Dat2::from_bytes(Arc::new(bytes));
        assert_eq!(data.read(&reference)?.as_slice(), payload);
    }
    Ok(())
}

#[test]
fn malformed_chains_return_errors() {
    let (reference, bytes, _) = archive(1);
    for fault in 0..7 {
        let mut data = bytes.clone();
        let mut reference = reference;
        match fault {
            0 => {
                data.pop();
            }
            1 => reference.sector = 0,
            2 => reference.sector = usize::MAX,
            3 => data[SECTOR_SIZE + 1] ^= 1, // archive ID
            4 => data[SECTOR_SIZE + 3] = 1,  // chunk number
            5 => data[SECTOR_SIZE + 7] = 6,  // index ID
            _ => data[SECTOR_SIZE + 6] = 1,  // cycle back to first chunk
        }
        assert!(
            Dat2::from_bytes(Arc::new(data)).read(&reference).is_err(),
            "fault {fault}"
        );
    }
}

#[test]
fn partial_index_entries_are_rejected() {
    for length in [1, 5, 7, 11] {
        assert!(Index::from_buffer(5, &vec![0; length]).is_err());
    }
    assert!(Index::from_buffer(5, &[0; 12]).is_ok());
}

#[test]
fn index_filename_mismatch_is_a_result() {
    assert!(Index::from_path(5, "main_file_cache.idx6").is_err());
}

#[test]
fn archive_reference_requires_exactly_six_bytes_and_bounded_length() {
    for length in [0, 5, 7] {
        assert!(ArchiveRef::from_buffer(1, 5, &vec![0; length]).is_err());
    }
    let reference = ArchiveRef {
        id: 1,
        index_id: 5,
        sector: 1,
        length: usize::MAX,
    };
    assert!(
        Dat2::from_bytes(Arc::new(Vec::new()))
            .read(&reference)
            .is_err()
    );
}

#[cfg(feature = "rs3")]
#[test]
fn malformed_lzma_is_an_error() {
    let buffer: osrs_filestore::codec::Buffer<osrs_filestore::codec::Encoded> =
        vec![3, 0, 0, 0, 1, 0, 0, 0, 1, 0].into();
    assert!(buffer.decode().is_err());
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_reads_match_owned_bytes_and_support_concurrent_readers()
-> Result<(), Box<dyn std::error::Error>> {
    let (reference, bytes, payload) = archive(70_000);
    let path = std::env::temp_dir().join(format!("rustosrs-filestore-{}.dat2", std::process::id()));
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let _cleanup = Cleanup(path.clone());
    std::fs::write(&path, &bytes)?;
    let native = Dat2::new(&path)?;
    let owned = Dat2::from_bytes(Arc::new(bytes));
    assert_eq!(
        native.read(&reference)?.as_slice(),
        owned.read(&reference)?.as_slice()
    );
    std::thread::scope(|scope| {
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let native = &native;
            let payload = &payload;
            tasks.push(scope.spawn(move || -> Result<(), osrs_filestore::Error> {
                for _ in 0..50 {
                    assert_eq!(native.read(&reference)?.as_slice(), payload);
                }
                Ok(())
            }));
        }
        for task in tasks {
            task.join().unwrap_or_else(|_| panic!("reader panicked"))?;
        }
        Ok::<_, osrs_filestore::Error>(())
    })?;
    // Truncation after opening is an ordinary I/O error, rather than a mapped-file fault.
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    assert!(native.read(&reference).is_err());
    Ok(())
}
