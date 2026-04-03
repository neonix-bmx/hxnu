#![allow(dead_code)]

use core::cell::UnsafeCell;

use crate::mm::compress;
use crate::mm::compress::store;

#[derive(Copy, Clone)]
pub struct PagerSummary {
    pub page_bytes: usize,
    pub store_capacity_pages: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum PagerInitError {
    AlreadyInitialized,
    CompressionStoreUnavailable,
}

impl PagerInitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "pager is already initialized",
            Self::CompressionStoreUnavailable => "compression store must be initialized before pager",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum PagerError {
    NotInitialized,
    Store(store::StoreError),
    VerifyFailed,
}

impl PagerError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "pager is not initialized",
            Self::Store(error) => error.as_str(),
            Self::VerifyFailed => "reclaim/restore verification failed",
        }
    }
}

#[derive(Copy, Clone)]
pub struct PagerEntry {
    pub page_id: u64,
    pub class: compress::CompressionClass,
    pub encoded_bytes: usize,
}

#[derive(Copy, Clone, Default)]
pub struct PagerStats {
    pub reclaim_requests: u64,
    pub reclaim_successes: u64,
    pub reclaim_failures: u64,
    pub restore_requests: u64,
    pub restore_successes: u64,
    pub restore_failures: u64,
    pub restore_misses: u64,
    pub verify_failures: u64,
    pub reclaimed_zero_pages: u64,
    pub reclaimed_same_pages: u64,
    pub reclaimed_sxrc_pages: u64,
    pub reclaimed_raw_pages: u64,
    pub restored_zero_pages: u64,
    pub restored_same_pages: u64,
    pub restored_sxrc_pages: u64,
    pub restored_raw_pages: u64,
    pub smoke_runs: u64,
    pub smoke_successes: u64,
}

#[derive(Copy, Clone)]
pub struct PagerSmokeSummary {
    pub tested_pages: u64,
    pub verified_pages: u64,
}

struct PagerState {
    initialized: bool,
    stats: PagerStats,
}

impl PagerState {
    const fn new() -> Self {
        Self {
            initialized: false,
            stats: PagerStats {
                reclaim_requests: 0,
                reclaim_successes: 0,
                reclaim_failures: 0,
                restore_requests: 0,
                restore_successes: 0,
                restore_failures: 0,
                restore_misses: 0,
                verify_failures: 0,
                reclaimed_zero_pages: 0,
                reclaimed_same_pages: 0,
                reclaimed_sxrc_pages: 0,
                reclaimed_raw_pages: 0,
                restored_zero_pages: 0,
                restored_same_pages: 0,
                restored_sxrc_pages: 0,
                restored_raw_pages: 0,
                smoke_runs: 0,
                smoke_successes: 0,
            },
        }
    }

    fn initialize(&mut self) -> Result<PagerSummary, PagerInitError> {
        if self.initialized {
            return Err(PagerInitError::AlreadyInitialized);
        }
        if !store::is_initialized() {
            return Err(PagerInitError::CompressionStoreUnavailable);
        }
        self.initialized = true;
        Ok(self.summary())
    }

    fn summary(&self) -> PagerSummary {
        let store_summary = store::summary();
        PagerSummary {
            page_bytes: compress::PAGE_BYTES,
            store_capacity_pages: store_summary.capacity_pages,
        }
    }

    fn reclaim_page(&mut self, page_id: u64, page: &[u8; compress::PAGE_BYTES]) -> Result<PagerEntry, PagerError> {
        if !self.initialized {
            return Err(PagerError::NotInitialized);
        }

        self.stats.reclaim_requests = self.stats.reclaim_requests.saturating_add(1);
        match store::store_page(page_id, page) {
            Ok(entry) => {
                self.stats.reclaim_successes = self.stats.reclaim_successes.saturating_add(1);
                self.account_reclaimed_class(entry.class);
                Ok(PagerEntry {
                    page_id: entry.page_id,
                    class: entry.class,
                    encoded_bytes: entry.encoded_bytes,
                })
            }
            Err(error) => {
                self.stats.reclaim_failures = self.stats.reclaim_failures.saturating_add(1);
                Err(PagerError::Store(error))
            }
        }
    }

    fn restore_page(
        &mut self,
        page_id: u64,
        out: &mut [u8; compress::PAGE_BYTES],
    ) -> Result<PagerEntry, PagerError> {
        if !self.initialized {
            return Err(PagerError::NotInitialized);
        }

        self.stats.restore_requests = self.stats.restore_requests.saturating_add(1);
        match store::load_page(page_id, out) {
            Ok(entry) => {
                self.stats.restore_successes = self.stats.restore_successes.saturating_add(1);
                self.account_restored_class(entry.class);
                Ok(PagerEntry {
                    page_id: entry.page_id,
                    class: entry.class,
                    encoded_bytes: entry.encoded_bytes,
                })
            }
            Err(error) => {
                self.stats.restore_failures = self.stats.restore_failures.saturating_add(1);
                if matches!(error, store::StoreError::NotFound) {
                    self.stats.restore_misses = self.stats.restore_misses.saturating_add(1);
                }
                Err(PagerError::Store(error))
            }
        }
    }

    fn run_bootstrap_smoke(&mut self) -> Result<PagerSmokeSummary, PagerError> {
        if !self.initialized {
            return Err(PagerError::NotInitialized);
        }

        self.stats.smoke_runs = self.stats.smoke_runs.saturating_add(1);
        self.verify_roundtrip(0x1000, &zero_page())?;
        self.verify_roundtrip(0x1001, &same_byte_page(0xAA))?;
        self.verify_roundtrip(0x1002, &pattern_page())?;
        self.verify_roundtrip(0x1003, &dictionary_word_page(0x0000_ffff))?;
        self.stats.smoke_successes = self.stats.smoke_successes.saturating_add(1);
        Ok(PagerSmokeSummary {
            tested_pages: 4,
            verified_pages: 4,
        })
    }

    fn stats(&self) -> PagerStats {
        self.stats
    }

    fn verify_roundtrip(
        &mut self,
        page_id: u64,
        page: &[u8; compress::PAGE_BYTES],
    ) -> Result<(), PagerError> {
        self.reclaim_page(page_id, page)?;

        let mut restored = [0u8; compress::PAGE_BYTES];
        self.restore_page(page_id, &mut restored)?;
        if page != &restored {
            self.stats.verify_failures = self.stats.verify_failures.saturating_add(1);
            return Err(PagerError::VerifyFailed);
        }
        Ok(())
    }

    fn account_reclaimed_class(&mut self, class: compress::CompressionClass) {
        match class {
            compress::CompressionClass::Zero => {
                self.stats.reclaimed_zero_pages = self.stats.reclaimed_zero_pages.saturating_add(1);
            }
            compress::CompressionClass::Same => {
                self.stats.reclaimed_same_pages = self.stats.reclaimed_same_pages.saturating_add(1);
            }
            compress::CompressionClass::Sxrc => {
                self.stats.reclaimed_sxrc_pages = self.stats.reclaimed_sxrc_pages.saturating_add(1);
            }
            compress::CompressionClass::Raw => {
                self.stats.reclaimed_raw_pages = self.stats.reclaimed_raw_pages.saturating_add(1);
            }
        }
    }

    fn account_restored_class(&mut self, class: compress::CompressionClass) {
        match class {
            compress::CompressionClass::Zero => {
                self.stats.restored_zero_pages = self.stats.restored_zero_pages.saturating_add(1);
            }
            compress::CompressionClass::Same => {
                self.stats.restored_same_pages = self.stats.restored_same_pages.saturating_add(1);
            }
            compress::CompressionClass::Sxrc => {
                self.stats.restored_sxrc_pages = self.stats.restored_sxrc_pages.saturating_add(1);
            }
            compress::CompressionClass::Raw => {
                self.stats.restored_raw_pages = self.stats.restored_raw_pages.saturating_add(1);
            }
        }
    }
}

struct GlobalPager(UnsafeCell<PagerState>);

unsafe impl Sync for GlobalPager {}

impl GlobalPager {
    const fn new() -> Self {
        Self(UnsafeCell::new(PagerState::new()))
    }

    fn get(&self) -> *mut PagerState {
        self.0.get()
    }
}

static PAGER: GlobalPager = GlobalPager::new();

pub fn initialize() -> Result<PagerSummary, PagerInitError> {
    unsafe { (*PAGER.get()).initialize() }
}

pub fn summary() -> PagerSummary {
    unsafe { (*PAGER.get()).summary() }
}

pub fn is_initialized() -> bool {
    unsafe { (*PAGER.get()).initialized }
}

pub fn reclaim_page(page_id: u64, page: &[u8; compress::PAGE_BYTES]) -> Result<PagerEntry, PagerError> {
    unsafe { (*PAGER.get()).reclaim_page(page_id, page) }
}

pub fn restore_page(page_id: u64, out: &mut [u8; compress::PAGE_BYTES]) -> Result<PagerEntry, PagerError> {
    unsafe { (*PAGER.get()).restore_page(page_id, out) }
}

pub fn run_bootstrap_smoke() -> Result<PagerSmokeSummary, PagerError> {
    unsafe { (*PAGER.get()).run_bootstrap_smoke() }
}

pub fn stats() -> PagerStats {
    unsafe { (*PAGER.get()).stats() }
}

fn zero_page() -> [u8; compress::PAGE_BYTES] {
    [0; compress::PAGE_BYTES]
}

fn same_byte_page(byte: u8) -> [u8; compress::PAGE_BYTES] {
    [byte; compress::PAGE_BYTES]
}

fn pattern_page() -> [u8; compress::PAGE_BYTES] {
    let mut page = [0u8; compress::PAGE_BYTES];
    let mut index = 0usize;
    while index < page.len() {
        page[index] = (index as u8).wrapping_mul(17).wrapping_add(5);
        index += 1;
    }
    page
}

fn dictionary_word_page(word: u32) -> [u8; compress::PAGE_BYTES] {
    let mut page = [0u8; compress::PAGE_BYTES];
    let bytes = word.to_le_bytes();
    let mut index = 0usize;
    while index + 4 <= page.len() {
        page[index..index + 4].copy_from_slice(&bytes);
        index += 4;
    }
    page
}
