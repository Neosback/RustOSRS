/// CRC-32 (IEEE), the checksum JS5 reference tables store for each group.
pub fn crc32(bytes: &[u8]) -> u32 {
    const fn table() -> [u32; 256] {
        let mut table = [0u32; 256];
        let mut n = 0;
        while n < 256 {
            let mut c = n as u32;
            let mut k = 0;
            while k < 8 {
                c = if c & 1 != 0 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
                k += 1;
            }
            table[n] = c;
            n += 1;
        }
        table
    }
    static TABLE: [u32; 256] = table();
    !bytes.iter().fold(!0u32, |crc, &byte| {
        TABLE[((crc ^ u32::from(byte)) & 0xff) as usize] ^ (crc >> 8)
    })
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn matches_the_standard_check_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"a"), 0xE8B7_BE43);
    }
}
