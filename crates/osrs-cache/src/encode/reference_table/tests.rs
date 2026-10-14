use super::*;

fn group(id: u32, crc: u32, version: u32, files: &[u32]) -> GroupEntry {
    GroupEntry {
        id,
        name_hash: None,
        crc,
        hash: None,
        whirlpool: None,
        sizes: None,
        version,
        file_ids: files.to_vec(),
        file_name_hashes: None,
    }
}

fn table(protocol: u8, flags: u8, groups: Vec<GroupEntry>) -> ReferenceTable {
    ReferenceTable {
        protocol,
        revision: (protocol >= 6).then_some(77),
        flags,
        groups,
    }
}

#[test]
fn plain_tables_round_trip_for_every_protocol() {
    for protocol in [5, 6, 7] {
        let original = table(
            protocol,
            0,
            vec![
                group(0, 0xdead_beef, 3, &[0]),
                group(3, 1, 1, &[0, 1, 2, 3, 4]),
                group(70_000 % 60_000, 9, 2, &[0, 5]),
            ],
        );
        let bytes = original.encode().unwrap_or_else(|e| unreachable!("{e}"));
        let parsed = ReferenceTable::parse(&bytes).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(parsed, original, "protocol {protocol}");
        assert_eq!(parsed.encode(), Ok(bytes));
    }
}

#[test]
fn every_optional_column_round_trips() {
    let mut a = group(1, 10, 1, &[0, 2]);
    a.name_hash = Some(-5);
    a.hash = Some(99);
    a.whirlpool = Some([7; 64]);
    a.sizes = Some((100, 400));
    a.file_name_hashes = Some(vec![1, -2]);
    let mut b = group(9, 11, 2, &[0]);
    b.name_hash = Some(0);
    b.hash = Some(-1);
    b.whirlpool = Some([8; 64]);
    b.sizes = Some((5, 6));
    b.file_name_hashes = Some(vec![3]);
    let original = table(7, 1 | 2 | 4 | 8, vec![a, b]);
    let bytes = original.encode().unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(ReferenceTable::parse(&bytes), Ok(original));
}

#[test]
fn big_smart_uses_two_bytes_below_32768_and_four_above() {
    let original = table(7, 0, vec![group(40_000, 1, 1, &[0, 100_000])]);
    let bytes = original.encode().unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(ReferenceTable::parse(&bytes), Ok(original));
    // protocol 7 with a 4-byte big smart for the group id delta 40_000.
    assert!(
        bytes
            .windows(4)
            .any(|w| w == (40_000u32 | 0x8000_0000).to_be_bytes())
    );
}

#[test]
fn invalid_tables_are_rejected_with_a_reason() {
    assert!(matches!(
        ReferenceTable::parse(&[7, 0, 0, 0, 1]),
        Err(ReferenceTableError::Truncated { .. })
    ));
    let mut bytes = table(6, 0, vec![group(0, 1, 1, &[0])])
        .encode()
        .unwrap_or_else(|e| unreachable!("{e}"));
    bytes.push(0);
    assert_eq!(
        ReferenceTable::parse(&bytes),
        Err(ReferenceTableError::TrailingBytes { remaining: 1 })
    );
    let descending = table(6, 0, vec![group(5, 1, 1, &[0]), group(2, 1, 1, &[0])]);
    assert!(descending.encode().is_err());
    // protocol 5 cannot hold a value above 16 bits.
    assert!(
        table(5, 0, vec![group(70_000, 1, 1, &[0])])
            .encode()
            .is_err()
    );
    // flags and columns must agree.
    assert!(table(6, 1, vec![group(0, 1, 1, &[0])]).encode().is_err());
}

#[test]
fn group_lookup_and_mutation_use_the_ascending_ids() {
    let mut t = table(6, 0, vec![group(1, 1, 1, &[0]), group(4, 2, 2, &[0])]);
    assert_eq!(t.group(4).map(|g| g.crc), Some(2));
    assert_eq!(t.group(2), None);
    if let Some(g) = t.group_mut(1) {
        g.version = 9;
    }
    assert_eq!(t.group(1).map(|g| g.version), Some(9));
}
