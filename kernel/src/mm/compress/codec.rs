use super::checksum;
use super::header::{EncodedHeader, HEADER_BYTES};
use super::profile_generated;
use super::{CompressionClass, CompressionError, EncodedPage, PAGE_BYTES};

const TOKEN_LITERAL: u8 = 0x00;
const TOKEN_DICT: u8 = 0x01;
const TOKEN_PATTERN: u8 = 0x02;
const TOKEN_REF_BYTES: usize = 3;
const DICT_DECODE_BYTES: usize = 4;

pub fn encode_page<'a>(
    page: &[u8; PAGE_BYTES],
    scratch: &'a mut [u8],
) -> Result<EncodedPage<'a>, CompressionError> {
    if scratch.len() < HEADER_BYTES {
        return Err(CompressionError::OutputTooSmall);
    }

    let (class, payload_len) = if is_zero_page(page) {
        (CompressionClass::Zero, 0usize)
    } else if is_same_page(page) {
        if scratch.len() < HEADER_BYTES + 1 {
            return Err(CompressionError::OutputTooSmall);
        }
        (CompressionClass::Same, {
            scratch[HEADER_BYTES] = page[0];
            1usize
        })
    } else {
        if scratch.len() < HEADER_BYTES + PAGE_BYTES {
            return Err(CompressionError::OutputTooSmall);
        }

        let payload_scratch = &mut scratch[HEADER_BYTES..];
        match encode_sxrc_payload(page, payload_scratch) {
            Ok(sxrc_len) if sxrc_len < PAGE_BYTES => (CompressionClass::Sxrc, sxrc_len),
            Ok(_) | Err(CompressionError::OutputTooSmall) => {
                payload_scratch[..PAGE_BYTES].copy_from_slice(page);
                (CompressionClass::Raw, PAGE_BYTES)
            }
            Err(error) => return Err(error),
        }
    };

    let encoded_len = HEADER_BYTES
        .checked_add(payload_len)
        .ok_or(CompressionError::InvalidPayloadLength)?;
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
        CompressionClass::Sxrc => decode_sxrc_payload(payload, out)?,
    }

    Ok(header.class())
}

fn is_zero_page(page: &[u8; PAGE_BYTES]) -> bool {
    page.iter().copied().all(|byte| byte == 0)
}

fn is_same_page(page: &[u8; PAGE_BYTES]) -> bool {
    let first = page[0];
    page.iter().copied().all(|byte| byte == first)
}

fn encode_sxrc_payload(page: &[u8; PAGE_BYTES], out: &mut [u8]) -> Result<usize, CompressionError> {
    let mut read = 0usize;
    let mut write = 0usize;
    let mut literal_start = 0usize;
    let mut literal_len = 0usize;

    while read < PAGE_BYTES {
        if let Some(token) = pick_token(page, read) {
            if literal_len > 0 {
                write = emit_literal_run(out, write, &page[literal_start..literal_start + literal_len])?;
                literal_len = 0;
            }

            write = match token {
                EncodeToken::Dictionary { id, decoded_len } => {
                    write = emit_ref_token(out, write, TOKEN_DICT, id)?;
                    read = read.saturating_add(decoded_len);
                    write
                }
                EncodeToken::Pattern { id, decoded_len } => {
                    write = emit_ref_token(out, write, TOKEN_PATTERN, id)?;
                    read = read.saturating_add(decoded_len);
                    write
                }
            };
            continue;
        }

        if literal_len == 0 {
            literal_start = read;
        }
        literal_len = literal_len.saturating_add(1);
        read = read.saturating_add(1);

        if literal_len == u8::MAX as usize {
            write = emit_literal_run(out, write, &page[literal_start..literal_start + literal_len])?;
            literal_len = 0;
        }
    }

    if literal_len > 0 {
        write = emit_literal_run(out, write, &page[literal_start..literal_start + literal_len])?;
    }
    Ok(write)
}

fn decode_sxrc_payload(payload: &[u8], out: &mut [u8; PAGE_BYTES]) -> Result<(), CompressionError> {
    let mut read = 0usize;
    let mut write = 0usize;

    while write < PAGE_BYTES {
        if read >= payload.len() {
            return Err(CompressionError::InvalidPayloadLength);
        }

        let token = payload[read];
        read = read.saturating_add(1);

        match token {
            TOKEN_LITERAL => {
                if read >= payload.len() {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                let len = payload[read] as usize;
                read = read.saturating_add(1);
                if len == 0 {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                if read.checked_add(len).is_none_or(|end| end > payload.len()) {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                if write.checked_add(len).is_none_or(|end| end > PAGE_BYTES) {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                out[write..write + len].copy_from_slice(&payload[read..read + len]);
                read = read.saturating_add(len);
                write = write.saturating_add(len);
            }
            TOKEN_DICT => {
                let id = read_u16(payload, &mut read)?;
                let value = lookup_dictionary_by_id(id).ok_or(CompressionError::InvalidPayloadLength)?;
                if write.checked_add(DICT_DECODE_BYTES).is_none_or(|end| end > PAGE_BYTES) {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                out[write..write + DICT_DECODE_BYTES].copy_from_slice(&value.to_le_bytes());
                write = write.saturating_add(DICT_DECODE_BYTES);
            }
            TOKEN_PATTERN => {
                let id = read_u16(payload, &mut read)?;
                let bytes = lookup_pattern_by_id(id).ok_or(CompressionError::InvalidPayloadLength)?;
                if write.checked_add(bytes.len()).is_none_or(|end| end > PAGE_BYTES) {
                    return Err(CompressionError::InvalidPayloadLength);
                }
                out[write..write + bytes.len()].copy_from_slice(bytes);
                write = write.saturating_add(bytes.len());
            }
            _ => return Err(CompressionError::InvalidPayloadLength),
        }
    }

    if read != payload.len() {
        return Err(CompressionError::InvalidPayloadLength);
    }
    Ok(())
}

fn emit_literal_run(out: &mut [u8], mut write: usize, bytes: &[u8]) -> Result<usize, CompressionError> {
    let mut offset = 0usize;
    while offset < bytes.len() {
        let chunk = (bytes.len() - offset).min(u8::MAX as usize);
        let needed = 2usize.saturating_add(chunk);
        if write.checked_add(needed).is_none_or(|end| end > out.len()) {
            return Err(CompressionError::OutputTooSmall);
        }
        out[write] = TOKEN_LITERAL;
        out[write + 1] = chunk as u8;
        out[write + 2..write + needed].copy_from_slice(&bytes[offset..offset + chunk]);
        write = write.saturating_add(needed);
        offset = offset.saturating_add(chunk);
    }
    Ok(write)
}

fn emit_ref_token(
    out: &mut [u8],
    write: usize,
    tag: u8,
    id: u16,
) -> Result<usize, CompressionError> {
    if write.checked_add(TOKEN_REF_BYTES).is_none_or(|end| end > out.len()) {
        return Err(CompressionError::OutputTooSmall);
    }
    out[write] = tag;
    out[write + 1..write + 3].copy_from_slice(&id.to_le_bytes());
    Ok(write + TOKEN_REF_BYTES)
}

fn read_u16(payload: &[u8], read: &mut usize) -> Result<u16, CompressionError> {
    if read.checked_add(2).is_none_or(|end| end > payload.len()) {
        return Err(CompressionError::InvalidPayloadLength);
    }
    let value = u16::from_le_bytes([payload[*read], payload[*read + 1]]);
    *read = read.saturating_add(2);
    Ok(value)
}

enum EncodeToken {
    Dictionary { id: u16, decoded_len: usize },
    Pattern { id: u16, decoded_len: usize },
}

fn pick_token(page: &[u8; PAGE_BYTES], offset: usize) -> Option<EncodeToken> {
    let mut best: Option<(usize, EncodeToken)> = None;

    if offset + DICT_DECODE_BYTES <= PAGE_BYTES {
        let value = u32::from_le_bytes([
            page[offset],
            page[offset + 1],
            page[offset + 2],
            page[offset + 3],
        ]);
        if let Some(id) = lookup_dictionary_by_value(value) {
            let savings = DICT_DECODE_BYTES.saturating_sub(TOKEN_REF_BYTES);
            if savings > 0 {
                best = Some((
                    savings,
                    EncodeToken::Dictionary {
                        id,
                        decoded_len: DICT_DECODE_BYTES,
                    },
                ));
            }
        }
    }

    if let Some((id, bytes)) = best_pattern_at(page, offset) {
        let decoded_len = bytes.len();
        let savings = decoded_len.saturating_sub(TOKEN_REF_BYTES);
        if savings > 0 {
            if best.as_ref().is_none_or(|(current_savings, _)| savings > *current_savings) {
                best = Some((savings, EncodeToken::Pattern { id, decoded_len }));
            }
        }
    }

    best.map(|(_, token)| token)
}

fn best_pattern_at(page: &[u8; PAGE_BYTES], offset: usize) -> Option<(u16, &'static [u8])> {
    let mut best: Option<(u16, &'static [u8])> = None;

    for (id, bytes) in profile_generated::HXNU_SXRC_STATIC_PATTERNS {
        if bytes.is_empty() {
            continue;
        }
        let len = bytes.len();
        if offset + len > PAGE_BYTES {
            continue;
        }
        if &page[offset..offset + len] != *bytes {
            continue;
        }
        if best.as_ref().is_none_or(|(_, current)| len > current.len()) {
            best = Some((*id, bytes));
        }
    }

    best
}

fn lookup_dictionary_by_value(value: u32) -> Option<u16> {
    for (id, word) in profile_generated::HXNU_SXRC_STATIC_DICTIONARY {
        if *word == value {
            return Some(*id);
        }
    }
    None
}

fn lookup_dictionary_by_id(id: u16) -> Option<u32> {
    for (entry_id, word) in profile_generated::HXNU_SXRC_STATIC_DICTIONARY {
        if *entry_id == id {
            return Some(*word);
        }
    }
    None
}

fn lookup_pattern_by_id(id: u16) -> Option<&'static [u8]> {
    for (entry_id, bytes) in profile_generated::HXNU_SXRC_STATIC_PATTERNS {
        if *entry_id == id {
            return Some(bytes);
        }
    }
    None
}
