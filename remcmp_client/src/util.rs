pub fn matches_checksum(checksum: u32, bin: &[u8]) -> bool {
    crc32fast::hash(bin) == checksum
}
