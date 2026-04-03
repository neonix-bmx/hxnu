const FNV1A32_OFFSET_BASIS: u32 = 0x811c_9dc5;
const FNV1A32_PRIME: u32 = 0x0100_0193;

pub fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut hash = FNV1A32_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV1A32_PRIME);
    }
    hash
}
