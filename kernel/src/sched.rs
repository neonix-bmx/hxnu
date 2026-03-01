use core::arch::asm;
use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::arch;
use crate::time;

const BOOTSTRAP_TARGET_TICKS: u64 = 3;
const BOOTSTRAP_TIMEOUT_NS: u64 = 500_000_000;

static BOOTSTRAP_ACTIVE: AtomicBool = AtomicBool::new(false);
static SCHEDULER_READY: AtomicBool = AtomicBool::new(false);
static SCHEDULER_TICKS: AtomicU64 = AtomicU64::new(0);

#[derive(Copy, Clone)]
pub struct SchedulerBootstrap {
    pub source: &'static str,
    pub vector: u8,
    pub divide_value: u32,
    pub initial_count: u32,
    pub ticks_observed: u64,
}

#[derive(Copy, Clone)]
pub enum SchedulerError {
    Timer(arch::x86_64::TimerError),
    Timeout,
}

impl SchedulerError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Timer(error) => error.as_str(),
            Self::Timeout => "scheduler bootstrap timed out waiting for periodic timer ticks",
        }
    }
}

pub fn bootstrap(
    hhdm_offset: u64,
    cpu_info: &arch::x86_64::CpuInfo,
) -> Result<SchedulerBootstrap, SchedulerError> {
    BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
    SCHEDULER_READY.store(false, Ordering::Release);
    SCHEDULER_TICKS.store(0, Ordering::Release);

    let timer = arch::x86_64::start_local_apic_periodic_timer(hhdm_offset, cpu_info)
        .map_err(SchedulerError::Timer)?;
    let deadline = time::uptime_nanoseconds().saturating_add(BOOTSTRAP_TIMEOUT_NS);

    BOOTSTRAP_ACTIVE.store(true, Ordering::Release);
    enable_interrupts();
    while SCHEDULER_TICKS.load(Ordering::Acquire) < BOOTSTRAP_TARGET_TICKS {
        if time::uptime_nanoseconds() >= deadline {
            BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
            disable_interrupts();
            arch::x86_64::mask_local_apic_timer();
            return Err(SchedulerError::Timeout);
        }

        unsafe {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
    disable_interrupts();

    BOOTSTRAP_ACTIVE.store(false, Ordering::Release);
    SCHEDULER_READY.store(true, Ordering::Release);
    Ok(SchedulerBootstrap {
        source: "local-apic-periodic",
        vector: timer.vector,
        divide_value: timer.divide_value,
        initial_count: timer.initial_count,
        ticks_observed: SCHEDULER_TICKS.load(Ordering::Acquire),
    })
}

pub fn on_timer_interrupt(_apic_tick: u64) {
    if !BOOTSTRAP_ACTIVE.load(Ordering::Acquire) && !SCHEDULER_READY.load(Ordering::Acquire) {
        return;
    }

    let tick = SCHEDULER_TICKS.fetch_add(1, Ordering::AcqRel) + 1;
    if tick <= BOOTSTRAP_TARGET_TICKS {
        kprintln!("HXNU: scheduler tick={}", tick);
    }
}

pub fn idle_loop() -> ! {
    kprintln!("HXNU: scheduler idle loop entered");
    loop {
        unsafe {
            asm!("sti; hlt", options(nomem, nostack));
        }
    }
}

fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nomem, nostack, preserves_flags));
    }
}

fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack, preserves_flags));
    }
}
