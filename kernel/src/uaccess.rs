use core::ptr;

const LOW_CANONICAL_MAX: usize = (1usize << 47) - 1;
const HIGH_CANONICAL_MIN: usize = !LOW_CANONICAL_MAX;
const MAX_COPY_BYTES: usize = 1 << 20;

#[derive(Copy, Clone)]
pub enum UserCopyError {
    NullPointer,
    RangeOverflow,
    NonCanonical,
    TooLarge,
}

impl UserCopyError {
    pub const fn as_errno(self) -> i64 {
        match self {
            Self::NullPointer | Self::RangeOverflow | Self::NonCanonical => 14,
            Self::TooLarge => 34,
        }
    }
}

pub fn copyin(user_ptr: usize, destination: &mut [u8]) -> Result<(), UserCopyError> {
    validate_user_range(user_ptr, destination.len())?;
    if destination.is_empty() {
        return Ok(());
    }

    unsafe {
        ptr::copy_nonoverlapping(user_ptr as *const u8, destination.as_mut_ptr(), destination.len());
    }
    Ok(())
}

pub fn copyout(source: &[u8], user_ptr: usize) -> Result<(), UserCopyError> {
    validate_user_range(user_ptr, source.len())?;
    if source.is_empty() {
        return Ok(());
    }

    unsafe {
        ptr::copy_nonoverlapping(source.as_ptr(), user_ptr as *mut u8, source.len());
    }
    Ok(())
}

pub fn validate_user_range(start: usize, len: usize) -> Result<(), UserCopyError> {
    if len > MAX_COPY_BYTES {
        return Err(UserCopyError::TooLarge);
    }
    if len == 0 {
        return Ok(());
    }
    if start == 0 {
        return Err(UserCopyError::NullPointer);
    }

    let end = start
        .checked_add(len - 1)
        .ok_or(UserCopyError::RangeOverflow)?;
    if !is_canonical_address(start) || !is_canonical_address(end) {
        return Err(UserCopyError::NonCanonical);
    }

    Ok(())
}

const fn is_canonical_address(address: usize) -> bool {
    address <= LOW_CANONICAL_MAX || address >= HIGH_CANONICAL_MIN
}
