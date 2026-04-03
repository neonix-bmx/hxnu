use super::{CompressionClass, CompressionError, PAGE_BYTES};

pub const HEADER_BYTES: usize = 16;
const MAGIC: u32 = u32::from_le_bytes(*b"HXCP");
const VERSION: u8 = 1;

#[derive(Copy, Clone)]
pub struct EncodedHeader {
    class: CompressionClass,
    payload_len: u16,
    checksum: u32,
}

impl EncodedHeader {
    pub const fn new(class: CompressionClass, payload_len: u16, checksum: u32) -> Self {
        Self {
            class,
            payload_len,
            checksum,
        }
    }

    pub const fn class(self) -> CompressionClass {
        self.class
    }

    pub const fn payload_len(self) -> u16 {
        self.payload_len
    }

    pub const fn checksum(self) -> u32 {
        self.checksum
    }

    pub fn encode_into(self, out: &mut [u8]) -> Result<(), CompressionError> {
        if out.len() < HEADER_BYTES {
            return Err(CompressionError::OutputTooSmall);
        }

        out[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        out[4] = VERSION;
        out[5] = self.class.id();
        out[6..8].copy_from_slice(&self.payload_len.to_le_bytes());
        out[8..10].copy_from_slice(&(PAGE_BYTES as u16).to_le_bytes());
        out[10..12].copy_from_slice(&0u16.to_le_bytes());
        out[12..16].copy_from_slice(&self.checksum.to_le_bytes());
        Ok(())
    }

    pub fn decode(input: &[u8]) -> Result<Self, CompressionError> {
        if input.len() < HEADER_BYTES {
            return Err(CompressionError::TruncatedInput);
        }

        let magic = read_u32_le(input, 0);
        if magic != MAGIC {
            return Err(CompressionError::InvalidHeaderMagic);
        }

        let version = input[4];
        if version != VERSION {
            return Err(CompressionError::UnsupportedHeaderVersion);
        }

        let class = CompressionClass::from_id(input[5])?;
        let payload_len = read_u16_le(input, 6);
        let decoded_len = read_u16_le(input, 8);
        if decoded_len != PAGE_BYTES as u16 {
            return Err(CompressionError::InvalidDecodedLength);
        }

        let checksum = read_u32_le(input, 12);
        Ok(Self {
            class,
            payload_len,
            checksum,
        })
    }
}

fn read_u16_le(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32_le(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
