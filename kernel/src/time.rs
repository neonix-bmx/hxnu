use core::arch::x86_64::{__cpuid, __cpuid_count, _rdtsc};
use core::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

const DEFAULT_TSC_HZ: u64 = 1_000_000_000;

#[derive(Copy, Clone)]
pub struct BootTimestamp {
    pub seconds: u64,
    pub nanoseconds: u32,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ClockSource {
    CpuidLeaf0x15,
    CpuidLeaf0x16,
    TscFallback,
}

impl ClockSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CpuidLeaf0x15 => "tsc-cpuid-0x15",
            Self::CpuidLeaf0x16 => "tsc-cpuid-0x16",
            Self::TscFallback => "tsc-fallback",
        }
    }

    pub fn is_estimated(self) -> bool {
        matches!(self, Self::TscFallback)
    }
}

static CLOCK_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CLOCK_START_TSC: AtomicU64 = AtomicU64::new(0);
static CLOCK_TSC_HZ: AtomicU64 = AtomicU64::new(DEFAULT_TSC_HZ);
static CLOCK_SOURCE: AtomicU8 = AtomicU8::new(ClockSource::TscFallback as u8);

pub fn initialize() -> ClockSource {
    let (tsc_hz, source) = detect_tsc_frequency().unwrap_or((DEFAULT_TSC_HZ, ClockSource::TscFallback));
    CLOCK_TSC_HZ.store(tsc_hz, Ordering::Relaxed);
    CLOCK_SOURCE.store(source as u8, Ordering::Relaxed);
    CLOCK_START_TSC.store(read_tsc(), Ordering::Relaxed);
    CLOCK_INITIALIZED.store(true, Ordering::Release);
    source
}

pub fn timestamp() -> BootTimestamp {
    if !CLOCK_INITIALIZED.load(Ordering::Acquire) {
        return BootTimestamp {
            seconds: 0,
            nanoseconds: 0,
        };
    }

    let start_tsc = CLOCK_START_TSC.load(Ordering::Relaxed);
    let tsc_hz = CLOCK_TSC_HZ.load(Ordering::Relaxed).max(1);
    let elapsed_cycles = read_tsc().wrapping_sub(start_tsc);
    let seconds = elapsed_cycles / tsc_hz;
    let cycle_remainder = elapsed_cycles % tsc_hz;
    let nanoseconds = ((cycle_remainder as u128) * 1_000_000_000u128 / (tsc_hz as u128)) as u32;

    BootTimestamp {
        seconds,
        nanoseconds,
    }
}

pub fn uptime_nanoseconds() -> u64 {
    let timestamp = timestamp();
    timestamp
        .seconds
        .saturating_mul(1_000_000_000)
        .saturating_add(timestamp.nanoseconds as u64)
}

fn detect_tsc_frequency() -> Option<(u64, ClockSource)> {
    let max_basic_leaf = __cpuid(0).eax;

    if max_basic_leaf >= 0x15 {
        let leaf_15 = __cpuid_count(0x15, 0);
        if leaf_15.eax != 0 && leaf_15.ebx != 0 && leaf_15.ecx != 0 {
            let numerator = (leaf_15.ecx as u128) * (leaf_15.ebx as u128);
            let denominator = leaf_15.eax as u128;
            let hz = (numerator / denominator) as u64;
            if hz != 0 {
                return Some((hz, ClockSource::CpuidLeaf0x15));
            }
        }
    }

    if max_basic_leaf >= 0x16 {
        let leaf_16 = __cpuid(0x16);
        if leaf_16.eax != 0 {
            let hz = (leaf_16.eax as u64).saturating_mul(1_000_000);
            if hz != 0 {
                return Some((hz, ClockSource::CpuidLeaf0x16));
            }
        }
    }

    None
}

#[inline]
fn read_tsc() -> u64 {
    unsafe { _rdtsc() }
}
