#![allow(dead_code)]

use core::cell::UnsafeCell;

mod checksum;
mod codec;
mod header;
mod profile_generated;

pub const PAGE_BYTES: usize = profile_generated::HXNU_SXRC_PAGE_SIZE;
const _PAGE_SIZE_MATCH: [(); PAGE_BYTES] = [(); crate::mm::frame::PAGE_SIZE as usize];
pub const MAX_ENCODED_PAGE_BYTES: usize = header::HEADER_BYTES + PAGE_BYTES;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum CompressionClass {
    Zero = 0,
    Same = 1,
    Sxrc = 2,
    Raw = 3,
}

impl CompressionClass {
    const fn id(self) -> u8 {
        self as u8
    }

    fn from_id(id: u8) -> Result<Self, CompressionError> {
        match id {
            0 => Ok(Self::Zero),
            1 => Ok(Self::Same),
            2 => Ok(Self::Sxrc),
            3 => Ok(Self::Raw),
            _ => Err(CompressionError::UnsupportedClass),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Same => "same",
            Self::Sxrc => "sxrc",
            Self::Raw => "raw",
        }
    }
}

#[derive(Copy, Clone)]
pub struct EncodedPage<'a> {
    class: CompressionClass,
    bytes: &'a [u8],
}

impl<'a> EncodedPage<'a> {
    pub(crate) fn new(class: CompressionClass, bytes: &'a [u8]) -> Self {
        Self { class, bytes }
    }

    pub fn class(self) -> CompressionClass {
        self.class
    }

    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub fn payload(self) -> &'a [u8] {
        self.bytes.get(header::HEADER_BYTES..).unwrap_or(&[])
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CompressionError {
    NotInitialized,
    OutputTooSmall,
    TruncatedInput,
    InvalidHeaderMagic,
    UnsupportedHeaderVersion,
    InvalidPayloadLength,
    InvalidDecodedLength,
    ChecksumMismatch,
    UnsupportedClass,
}

impl CompressionError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "compression runtime is not initialized",
            Self::OutputTooSmall => "encode output buffer is too small",
            Self::TruncatedInput => "encoded page buffer is truncated",
            Self::InvalidHeaderMagic => "encoded page header magic is invalid",
            Self::UnsupportedHeaderVersion => "encoded page header version is unsupported",
            Self::InvalidPayloadLength => "encoded payload length is invalid",
            Self::InvalidDecodedLength => "encoded decoded-length field is invalid",
            Self::ChecksumMismatch => "encoded payload checksum mismatch",
            Self::UnsupportedClass => "compression class is unsupported by active backend",
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct CompressionStats {
    pub encoded_pages: u64,
    pub decoded_pages: u64,
    pub zero_pages: u64,
    pub same_pages: u64,
    pub sxrc_pages: u64,
    pub raw_pages: u64,
    pub fallback_raw_pages: u64,
    pub encode_failures: u64,
    pub decode_failures: u64,
}

pub trait CompressionBackend {
    fn backend_name(&self) -> &'static str;
    fn profile_name(&self) -> &'static str;
    fn profile_version(&self) -> u32;
    fn encode_page<'a>(
        &mut self,
        page: &[u8; PAGE_BYTES],
        scratch: &'a mut [u8],
    ) -> Result<EncodedPage<'a>, CompressionError>;
    fn decode_page(
        &mut self,
        encoded: EncodedPage<'_>,
        out: &mut [u8; PAGE_BYTES],
    ) -> Result<(), CompressionError>;
    fn stats(&self) -> CompressionStats;
}

#[derive(Copy, Clone)]
pub struct CompressionRuntimeSummary {
    pub backend: &'static str,
    pub profile: &'static str,
    pub profile_version: u32,
    pub page_bytes: usize,
    pub compression_unit_bytes: usize,
    pub little_endian: bool,
    pub dynamic_patterns: bool,
    pub static_dictionary_entries: usize,
    pub static_pattern_entries: usize,
    pub encoded_header_bytes: usize,
    pub max_encoded_page_bytes: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum CompressionInitError {
    AlreadyInitialized,
}

impl CompressionInitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "compression runtime is already initialized",
        }
    }
}

pub struct NullBackend {
    stats: CompressionStats,
}

impl NullBackend {
    pub const fn new() -> Self {
        Self {
            stats: CompressionStats {
                encoded_pages: 0,
                decoded_pages: 0,
                zero_pages: 0,
                same_pages: 0,
                sxrc_pages: 0,
                raw_pages: 0,
                fallback_raw_pages: 0,
                encode_failures: 0,
                decode_failures: 0,
            },
        }
    }

    fn account_encoded_class(&mut self, class: CompressionClass) {
        match class {
            CompressionClass::Zero => {
                self.stats.zero_pages = self.stats.zero_pages.saturating_add(1);
            }
            CompressionClass::Same => {
                self.stats.same_pages = self.stats.same_pages.saturating_add(1);
            }
            CompressionClass::Sxrc => {
                self.stats.sxrc_pages = self.stats.sxrc_pages.saturating_add(1);
            }
            CompressionClass::Raw => {
                self.stats.raw_pages = self.stats.raw_pages.saturating_add(1);
                self.stats.fallback_raw_pages = self.stats.fallback_raw_pages.saturating_add(1);
            }
        }
    }
}

impl CompressionBackend for NullBackend {
    fn backend_name(&self) -> &'static str {
        "null"
    }

    fn profile_name(&self) -> &'static str {
        profile_generated::HXNU_SXRC_PROFILE_NAME
    }

    fn profile_version(&self) -> u32 {
        profile_generated::HXNU_SXRC_PROFILE_VERSION
    }

    fn encode_page<'a>(
        &mut self,
        page: &[u8; PAGE_BYTES],
        scratch: &'a mut [u8],
    ) -> Result<EncodedPage<'a>, CompressionError> {
        self.stats.encoded_pages = self.stats.encoded_pages.saturating_add(1);

        match codec::encode_page(page, scratch) {
            Ok(encoded) => {
                self.account_encoded_class(encoded.class());
                Ok(encoded)
            }
            Err(error) => {
                self.stats.encode_failures = self.stats.encode_failures.saturating_add(1);
                Err(error)
            }
        }
    }

    fn decode_page(
        &mut self,
        encoded: EncodedPage<'_>,
        out: &mut [u8; PAGE_BYTES],
    ) -> Result<(), CompressionError> {
        match codec::decode_page(encoded.bytes(), out) {
            Ok(_) => {
                self.stats.decoded_pages = self.stats.decoded_pages.saturating_add(1);
                Ok(())
            }
            Err(error) => {
                self.stats.decode_failures = self.stats.decode_failures.saturating_add(1);
                Err(error)
            }
        }
    }

    fn stats(&self) -> CompressionStats {
        self.stats
    }
}

struct CompressionRuntime {
    initialized: bool,
    backend: NullBackend,
}

impl CompressionRuntime {
    const fn new() -> Self {
        Self {
            initialized: false,
            backend: NullBackend::new(),
        }
    }

    fn initialize(&mut self) -> Result<CompressionRuntimeSummary, CompressionInitError> {
        if self.initialized {
            return Err(CompressionInitError::AlreadyInitialized);
        }
        self.initialized = true;
        Ok(self.summary())
    }

    fn summary(&self) -> CompressionRuntimeSummary {
        CompressionRuntimeSummary {
            backend: self.backend.backend_name(),
            profile: self.backend.profile_name(),
            profile_version: self.backend.profile_version(),
            page_bytes: PAGE_BYTES,
            compression_unit_bytes: profile_generated::HXNU_SXRC_COMPRESSION_UNIT_BYTES,
            little_endian: profile_generated::HXNU_SXRC_ENDIAN_LITTLE,
            dynamic_patterns: profile_generated::HXNU_SXRC_ENABLE_DYNAMIC_PATTERNS,
            static_dictionary_entries: profile_generated::HXNU_SXRC_STATIC_DICTIONARY.len(),
            static_pattern_entries: profile_generated::HXNU_SXRC_STATIC_PATTERNS.len(),
            encoded_header_bytes: header::HEADER_BYTES,
            max_encoded_page_bytes: MAX_ENCODED_PAGE_BYTES,
        }
    }

    fn encode_page<'a>(
        &mut self,
        page: &[u8; PAGE_BYTES],
        scratch: &'a mut [u8],
    ) -> Result<EncodedPage<'a>, CompressionError> {
        if !self.initialized {
            return Err(CompressionError::NotInitialized);
        }
        self.backend.encode_page(page, scratch)
    }

    fn decode_page(
        &mut self,
        encoded: EncodedPage<'_>,
        out: &mut [u8; PAGE_BYTES],
    ) -> Result<(), CompressionError> {
        if !self.initialized {
            return Err(CompressionError::NotInitialized);
        }
        self.backend.decode_page(encoded, out)
    }

    fn stats(&self) -> CompressionStats {
        self.backend.stats()
    }
}

struct GlobalCompressionRuntime(UnsafeCell<CompressionRuntime>);

unsafe impl Sync for GlobalCompressionRuntime {}

impl GlobalCompressionRuntime {
    const fn new() -> Self {
        Self(UnsafeCell::new(CompressionRuntime::new()))
    }

    fn get(&self) -> *mut CompressionRuntime {
        self.0.get()
    }
}

static COMPRESSION_RUNTIME: GlobalCompressionRuntime = GlobalCompressionRuntime::new();

pub fn initialize() -> Result<CompressionRuntimeSummary, CompressionInitError> {
    unsafe { (*COMPRESSION_RUNTIME.get()).initialize() }
}

pub fn summary() -> CompressionRuntimeSummary {
    unsafe { (*COMPRESSION_RUNTIME.get()).summary() }
}

pub fn is_initialized() -> bool {
    unsafe { (*COMPRESSION_RUNTIME.get()).initialized }
}

pub fn encode_page<'a>(
    page: &[u8; PAGE_BYTES],
    scratch: &'a mut [u8],
) -> Result<EncodedPage<'a>, CompressionError> {
    unsafe { (*COMPRESSION_RUNTIME.get()).encode_page(page, scratch) }
}

pub fn decode_page(
    encoded: EncodedPage<'_>,
    out: &mut [u8; PAGE_BYTES],
) -> Result<(), CompressionError> {
    unsafe { (*COMPRESSION_RUNTIME.get()).decode_page(encoded, out) }
}

pub fn stats() -> CompressionStats {
    unsafe { (*COMPRESSION_RUNTIME.get()).stats() }
}
