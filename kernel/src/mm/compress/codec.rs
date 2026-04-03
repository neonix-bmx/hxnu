use super::checksum;
use super::header::{EncodedHeader, HEADER_BYTES};
use super::{CompressionClass, CompressionError, EncodedPage, PAGE_BYTES};

pub fn encode_page<'a>(
    page: &[u8; PAGE_BYTES],
    scratch: &'a mut [u8],
) -> Result<EncodedPage<'a>, CompressionError> {
    let class = classify_page(page);
    let payload_len = payload_len_for_class(class);
    let encoded_len = HEADER_BYTES
        .checked_add(payload_len)
        .ok_or(CompressionError::InvalidPayloadLength)?;
    if scratch.len() < encoded_len {
        return Err(CompressionError::OutputTooSmall);
    }

    match class {
        CompressionClass::Zero => {}
        CompressionClass::Same => {
            scratch[HEADER_BYTES] = page[0];
        }
        CompressionClass::Raw => {
            scratch[HEADER_BYTES..encoded_len].copy_from_slice(page);
        }
        CompressionClass::Sxrc => {
            return Err(CompressionError::UnsupportedClass);
        }
    }

    let payload = &scratch[HEADER_BYTES..encoded_len];
    let header = EncodedHeader::new(class, payload_len as u16, checksum::fnv1a32(payload));
    header.encode_into(scratch)?;
    Ok(EncodedPage::new(class, &scratch[..encoded_len]))
}

pub fn decode_page(encoded: &[u8], out: &mut [u8; PAGE_BYTES]) -> Result<CompressionClass, CompressionError> {
    let header = EncodedHeader::decode(encoded)?;
    let payload_len = usize::from(header.payload_len());
    let encoded_len = HEADER_BYTES
        .checked_add(payload_len)
        .ok_or(CompressionError::InvalidPayloadLength)?;
    if encoded.len() != encoded_len {
        return Err(CompressionError::InvalidPayloadLength);
    }

    let payload = &encoded[HEADER_BYTES..encoded_len];
    if checksum::fnv1a32(payload) != header.checksum() {
        return Err(CompressionError::ChecksumMismatch);
    }

    match header.class() {
        CompressionClass::Zero => {
            if !payload.is_empty() {
                return Err(CompressionError::InvalidPayloadLength);
            }
            out.fill(0);
        }
        CompressionClass::Same => {
            if payload.len() != 1 {
                return Err(CompressionError::InvalidPayloadLength);
            }
            out.fill(payload[0]);
        }
        CompressionClass::Raw => {
            if payload.len() != PAGE_BYTES {
                return Err(CompressionError::InvalidPayloadLength);
            }
            out.copy_from_slice(payload);
        }
        CompressionClass::Sxrc => return Err(CompressionError::UnsupportedClass),
    }

    Ok(header.class())
}

fn classify_page(page: &[u8; PAGE_BYTES]) -> CompressionClass {
    let first = page[0];
    if page.iter().copied().all(|byte| byte == 0) {
        return CompressionClass::Zero;
    }
    if page.iter().copied().all(|byte| byte == first) {
        return CompressionClass::Same;
    }
    CompressionClass::Raw
}

fn payload_len_for_class(class: CompressionClass) -> usize {
    match class {
        CompressionClass::Zero => 0,
        CompressionClass::Same => 1,
        CompressionClass::Sxrc => 0,
        CompressionClass::Raw => PAGE_BYTES,
    }
}
