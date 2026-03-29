mod apic;
mod context;
mod cpuid;
mod cpu;
mod early_map;
mod gdt;
mod interrupts;

use core::arch::x86_64::CpuidResult;

#[derive(Copy, Clone)]
pub enum ExceptionSelfTest {
    Breakpoint,
    PageFault,
    GeneralProtectionFault,
}

#[derive(Copy, Clone)]
pub struct SyscallSelfTest {
    pub linux_write_result: i64,
    pub linux_openat_result: i64,
    pub linux_read_result: i64,
    pub linux_close_result: i64,
    pub linux_getpid_result: i64,
    pub ghost_open_result: i64,
    pub ghost_read_result: i64,
    pub ghost_close_result: i64,
    pub ghost_gettid_result: i64,
    pub hxnu_open_result: i64,
    pub hxnu_read_result: i64,
    pub hxnu_close_result: i64,
    pub hxnu_abi_version_result: i64,
}

pub use apic::{PeriodicTimer, TimerBringUp, TimerError};
pub use context::TaskContext;
pub use cpu::CpuInfo;
pub use early_map::MapError;

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

pub fn cpuid(leaf: u32) -> CpuidResult {
    cpuid::query(leaf)
}

pub fn cpuid_count(leaf: u32, subleaf: u32) -> CpuidResult {
    cpuid::query_count(leaf, subleaf)
}

pub fn max_basic_cpuid_leaf() -> u32 {
    cpuid::max_basic_leaf()
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

pub fn ensure_physical_region_mapped(
    hhdm_offset: u64,
    physical_address: u64,
    length: usize,
    extra_flags: u64,
) -> Result<u64, MapError> {
    early_map::ensure_region_mapped(hhdm_offset, physical_address, length, extra_flags)
}

pub fn initialize_kernel_thread_context(
    context: &mut TaskContext,
    stack: &'static mut [u8],
    entry: extern "C" fn() -> !,
) {
    context::initialize_kernel_thread(context, stack, entry)
}

pub unsafe fn switch_context(current: &mut TaskContext, next: &TaskContext) -> ! {
    unsafe { context::switch(current, next) }
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

pub fn run_syscall_self_test() -> SyscallSelfTest {
    let result = interrupts::run_syscall_self_test();
    SyscallSelfTest {
        linux_write_result: result.linux_write_result,
        linux_openat_result: result.linux_openat_result,
        linux_read_result: result.linux_read_result,
        linux_close_result: result.linux_close_result,
        linux_getpid_result: result.linux_getpid_result,
        ghost_open_result: result.ghost_open_result,
        ghost_read_result: result.ghost_read_result,
        ghost_close_result: result.ghost_close_result,
        ghost_gettid_result: result.ghost_gettid_result,
        hxnu_open_result: result.hxnu_open_result,
        hxnu_read_result: result.hxnu_read_result,
        hxnu_close_result: result.hxnu_close_result,
        hxnu_abi_version_result: result.hxnu_abi_version_result,
    }
}
