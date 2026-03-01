mod apic;
mod cpu;
mod gdt;
mod interrupts;

#[derive(Copy, Clone)]
pub enum ExceptionSelfTest {
    Breakpoint,
    PageFault,
    GeneralProtectionFault,
}

pub use apic::{PeriodicTimer, TimerBringUp, TimerError};
pub use cpu::CpuInfo;

pub fn initialize() {
    gdt::initialize();
    interrupts::initialize();
}

pub fn segment_selectors() -> gdt::SegmentSelectors {
    gdt::read_segment_selectors()
}

pub fn probe_cpu() -> cpu::CpuInfo {
    cpu::probe()
}

pub fn initialize_local_apic_timer(
    hhdm_offset: u64,
    cpu_info: &CpuInfo,
) -> Result<TimerBringUp, TimerError> {
    apic::initialize_timer(hhdm_offset, cpu_info)
}

pub fn start_local_apic_periodic_timer(
    hhdm_offset: u64,
    cpu_info: &CpuInfo,
) -> Result<PeriodicTimer, TimerError> {
    apic::start_periodic_timer(hhdm_offset, cpu_info)
}

pub fn mask_local_apic_timer() {
    apic::mask_periodic_timer();
}

pub fn run_exception_self_test(test: ExceptionSelfTest) {
    match test {
        ExceptionSelfTest::Breakpoint => interrupts::trigger_breakpoint(),
        ExceptionSelfTest::PageFault => interrupts::trigger_page_fault(),
        ExceptionSelfTest::GeneralProtectionFault => interrupts::trigger_general_protection_fault(),
    }
}
