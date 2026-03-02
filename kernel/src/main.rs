#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod acpi;
mod arch;
mod fb;
#[macro_use]
mod log;
mod limine;
mod mm;
mod panic;
mod power;
mod sched;
mod serial;
mod time;
mod tty;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::arch::asm;

const SELF_TEST: Option<SelfTest> = selected_self_test();

#[derive(Copy, Clone)]
enum SelfTest {
    Breakpoint,
    PageFault,
    GeneralProtectionFault,
    Panic,
    PowerReset,
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "C" fn _start() -> ! {
    serial::init();
    let clock_source = time::initialize();

    if !limine::base_revision_supported() {
        kprintln!("HXNU: unsupported Limine base revision");
        halt();
    }

    kprintln!("HXNU: x86_64 early bootstrap");
    kprintln!(
        "HXNU: log clock source = {}{}",
        clock_source.as_str(),
        if clock_source.is_estimated() { " (estimated)" } else { "" }
    );
    kprintln!("HXNU: Limine protocol handshake ok");

    let hhdm_offset = match limine::hhdm_offset() {
        Some(offset) => {
            kprintln!("HXNU: HHDM offset = {offset:#018x}");
            offset
        }
        None => {
            kprintln!("HXNU: HHDM response missing");
            halt();
        }
    };

    match limine::framebuffer() {
        Some(framebuffer) => match fb::initialize(framebuffer) {
            Ok(summary) => {
                let tty = tty::initialize(true);
                kprintln_style!(
                    crate::tty::ConsoleStyle::Accent,
                    "HXNU: framebuffer online mode={}x{} pitch={} bpp={}",
                    summary.width,
                    summary.height,
                    summary.pitch,
                    summary.bpp,
                );
                kprintln_style!(
                    crate::tty::ConsoleStyle::Muted,
                    "HXNU: framebuffer probe background={:#010x} accent={:#010x}",
                    summary.sample_background,
                    summary.sample_accent,
                );
                if let Some(ink) = fb::console_probe() {
                    kprintln_style!(
                        crate::tty::ConsoleStyle::Accent,
                        "HXNU: framebuffer console probe ink={:#010x}",
                        ink
                    );
                }
                kprintln_style!(
                    crate::tty::ConsoleStyle::Success,
                    "HXNU: tty console online id={} outputs={} framebuffer={}",
                    tty.console_id,
                    tty.output_count,
                    yes_no(tty.framebuffer_output),
                );
            }
            Err(error) => {
                let tty = tty::initialize(false);
                kprintln_style!(
                    crate::tty::ConsoleStyle::Error,
                    "HXNU: framebuffer offline reason={}",
                    error.as_str()
                );
                kprintln!(
                    "HXNU: tty console online id={} outputs={} framebuffer={}",
                    tty.console_id,
                    tty.output_count,
                    yes_no(tty.framebuffer_output),
                );
            }
        },
        None => {
            let tty = tty::initialize(false);
            kprintln!("HXNU: framebuffer response missing");
            kprintln!(
                "HXNU: tty console online id={} outputs={} framebuffer={}",
                tty.console_id,
                tty.output_count,
                yes_no(tty.framebuffer_output),
            );
        }
    }

    match limine::memory_map() {
        Some(memory_map) => {
            if memory_map.is_empty() {
                kprintln!("HXNU: memmap is empty");
                halt();
            }
            kprintln!("HXNU: memmap entries = {}", memory_map.len());
            if let Some(region) = memory_map.iter().find(|entry| entry.is_usable()) {
                kprintln!(
                    "HXNU: first usable region = base {:#018x}, size {} KiB",
                    region.base,
                    region.length / 1024
                );
            }

            match mm::frame::initialize(&memory_map) {
                Ok(stats) => {
                    kprintln!("HXNU: usable memory = {} KiB", stats.total_bytes / 1024);
                    kprintln!("HXNU: frame regions = {}", stats.usable_regions);
                    kprintln!("HXNU: allocatable frames = {}", stats.allocatable_bytes / mm::frame::PAGE_SIZE);
                    kprintln!(
                        "HXNU: allocatable memory = {} KiB",
                        stats.allocatable_bytes / 1024
                    );
                    match mm::frame::allocate_frame() {
                        Some(frame) => kprintln!(
                            "HXNU: bootstrap frame = {:#018x}",
                            frame.start_address()
                        ),
                        None => {
                            kprintln!("HXNU: frame allocator returned no frame");
                            halt();
                        }
                    }
                    let stats = mm::frame::stats();
                    kprintln!("HXNU: allocated frames = {}", stats.allocated_frames);
                }
                Err(error) => {
                    kprintln!("HXNU: frame allocator init failed: {:?}", error);
                    halt();
                }
            }
        }
        None => kprintln!("HXNU: memmap response missing"),
    }

    match mm::heap::initialize(hhdm_offset) {
        Ok(stats) => {
            kprintln!("HXNU: heap start = {:#018x}", stats.start);
            kprintln!("HXNU: heap size = {} KiB", stats.size_bytes / 1024);

            let boxed = Box::new(0x4858_4e55_u32);
            kprintln!("HXNU: boxed value = {:#010x}", *boxed);

            let mut values = Vec::new();
            values.push(3_u64);
            values.push(1_u64);
            values.push(4_u64);
            values.push(1_u64);
            values.push(5_u64);
            let sum: u64 = values.iter().copied().sum();
            kprintln!("HXNU: vec len = {}, sum = {}", values.len(), sum);

            let stats = mm::heap::stats();
            kprintln!("HXNU: heap used = {} bytes", stats.used_bytes);
            kprintln!("HXNU: heap allocations = {}", stats.allocation_count);
        }
        Err(error) => {
            kprintln!("HXNU: heap init failed: {:?}", error);
            halt();
        }
    }

    arch::x86_64::initialize();
    let selectors = arch::x86_64::segment_selectors();
    kprintln!(
        "HXNU: x86_64 descriptor tables loaded cs={:#06x} ds={:#06x} es={:#06x} fs={:#06x} gs={:#06x} ss={:#06x}",
        selectors.cs,
        selectors.ds,
        selectors.es,
        selectors.fs,
        selectors.gs,
        selectors.ss,
    );
    let cpu_info = arch::x86_64::probe_cpu();
    kprintln!(
        "HXNU: cpu local-apic={} x2apic={} tsc-deadline={}",
        yes_no(cpu_info.local_apic_supported),
        yes_no(cpu_info.x2apic_supported),
        yes_no(cpu_info.tsc_deadline_supported),
    );
    if cpu_info.local_apic_supported {
        kprintln!(
            "HXNU: apic base={:#010x} enabled={} x2apic-mode={} bsp={}",
            cpu_info.apic_base,
            yes_no(cpu_info.apic_global_enabled),
            yes_no(cpu_info.x2apic_enabled),
            yes_no(cpu_info.bootstrap_processor),
        );
    }
    match arch::x86_64::initialize_local_apic_timer(hhdm_offset, &cpu_info) {
        Ok(timer) => kprintln!(
            "HXNU: apic timer online vector={:#04x} divide={} initial-count={} ticks={}",
            timer.vector,
            timer.divide_value,
            timer.initial_count,
            timer.ticks_observed,
        ),
        Err(error) => kprintln!("HXNU: apic timer offline reason={}", error.as_str()),
    }

    match limine::rsdp_address() {
        Some(rsdp_address) => {
            kprintln!("HXNU: acpi rsdp response @ {:#010x}", rsdp_address);
            match acpi::discover(hhdm_offset, rsdp_address) {
                Ok(discovery) => {
                    kprintln_style!(
                        crate::tty::ConsoleStyle::Accent,
                        "HXNU: acpi online revision={} oem={} rsdp={:#010x} root={} @ {:#010x}",
                        discovery.revision,
                        acpi::oem_id_str(&discovery.oem_id),
                        discovery.rsdp_address,
                        discovery.root_kind.as_str(),
                        discovery.root_address,
                    );
                    kprintln!(
                        "HXNU: acpi tables total={} valid={} invalid={} madt={} fadt={}",
                        discovery.table_count,
                        discovery.valid_table_count,
                        discovery.invalid_table_count,
                        yes_no(discovery.madt.is_some()),
                        yes_no(discovery.fadt.is_some()),
                    );
                    if let Some(ref madt) = discovery.madt {
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Accent,
                            "HXNU: acpi madt lapic={:#010x} flags={:#010x} cpus-enabled={}/{} ioapics={} iso={} x2apic-cpus={}",
                            madt.local_apic_address,
                            madt.flags,
                            madt.enabled_processor_count(),
                            madt.total_processor_count(),
                            madt.io_apics.len(),
                            madt.interrupt_source_overrides.len(),
                            madt.local_x2apic_count(),
                        );
                        if let Some(processor) = madt.processors.first() {
                            kprintln!(
                                "HXNU: acpi cpu0 uid={} apic={} mode={} enabled={} online-capable={}",
                                processor.processor_uid,
                                processor.apic_id,
                                processor.apic_mode(),
                                yes_no(processor.enabled),
                                yes_no(processor.online_capable),
                            );
                        }
                        if let Some(io_apic) = madt.io_apics.first() {
                            kprintln!(
                                "HXNU: acpi ioapic0 id={} addr={:#010x} gsi-base={}",
                                io_apic.io_apic_id,
                                io_apic.address,
                                io_apic.global_system_interrupt_base,
                            );
                        }
                        if let Some(override_entry) = madt.interrupt_source_overrides.first() {
                            kprintln!(
                                "HXNU: acpi iso0 source={} gsi={} flags={:#06x}",
                                override_entry.source,
                                override_entry.global_system_interrupt,
                                override_entry.flags,
                            );
                        }
                    }
                    if let Some(ref fadt) = discovery.fadt {
                        power::configure(hhdm_offset, fadt);
                        kprintln!(
                            "HXNU: acpi fadt revision={} length={} profile={} sci={} smi-cmd={:#x}",
                            fadt.revision,
                            fadt.length,
                            fadt.preferred_pm_profile.as_str(),
                            fadt.sci_interrupt,
                            fadt.smi_command_port,
                        );
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Warning,
                            "HXNU: acpi power reset={} hw-reduced={} pm1a-ctl={:#x} pm1b-ctl={:#x}",
                            yes_no(fadt.reset_supported()),
                            yes_no(fadt.hardware_reduced()),
                            fadt.pm1a_control_block,
                            fadt.pm1b_control_block,
                        );
                        if let Some(reset_register) = fadt.reset_register {
                            kprintln!(
                                "HXNU: acpi reset-reg space={} width={} offset={} access={} addr={:#x} value={:#04x}",
                                reset_register.address_space_str(),
                                reset_register.bit_width,
                                reset_register.bit_offset,
                                reset_register.access_size,
                                reset_register.address,
                                fadt.reset_value,
                            );
                        }
                        kprintln!(
                            "HXNU: acpi boot-arch flags={:#06x} acpi-enable={:#04x} acpi-disable={:#04x}",
                            fadt.boot_architecture_flags,
                            fadt.acpi_enable,
                            fadt.acpi_disable,
                        );
                    }
                }
                Err(error) => kprintln!("HXNU: acpi offline reason={}", error.as_str()),
            }
        }
        None => kprintln!("HXNU: acpi rsdp response missing"),
    }

    if let Some(test) = SELF_TEST {
        match test {
            SelfTest::Breakpoint => {
                kprintln!("HXNU: running exception self-test = breakpoint");
                arch::x86_64::run_exception_self_test(arch::x86_64::ExceptionSelfTest::Breakpoint);
                kprintln!("HXNU: breakpoint handler returned");
            }
            SelfTest::PageFault => {
                kprintln!("HXNU: running exception self-test = page-fault");
                arch::x86_64::run_exception_self_test(arch::x86_64::ExceptionSelfTest::PageFault);
                kprintln!("HXNU: page-fault self-test unexpectedly returned");
                halt();
            }
            SelfTest::GeneralProtectionFault => {
                kprintln!("HXNU: running exception self-test = general-protection-fault");
                arch::x86_64::run_exception_self_test(
                    arch::x86_64::ExceptionSelfTest::GeneralProtectionFault,
                );
                kprintln!("HXNU: gpf self-test unexpectedly returned");
                halt();
            }
            SelfTest::Panic => {
                kprintln!("HXNU: running kernel self-test = panic");
                panic!("requested kernel panic self-test");
            }
            SelfTest::PowerReset => {
                let capability = power::reset_capability();
                kprintln!(
                    "HXNU: running kernel self-test = power-reset supported={} space={} addr={:#x} value={:#04x}",
                    yes_no(capability.supported),
                    capability.address_space_str(),
                    capability.address,
                    capability.value,
                );
                match power::reboot() {
                    Ok(()) => power::halt_forever(),
                    Err(error) => {
                        kprintln_style!(
                            crate::tty::ConsoleStyle::Error,
                            "HXNU: power reset self-test failed reason={}",
                            error.as_str()
                        );
                        halt();
                    }
                }
            }
        }
    }

    match sched::bootstrap(hhdm_offset, &cpu_info) {
        Ok(state) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: scheduler bootstrap online source={} vector={:#04x} divide={} initial-count={} ticks={} threads={} runqueue={} current={}#{} role={} switches={} bootstrap-id={} idle-id={}",
            state.source,
            state.vector,
            state.divide_value,
            state.initial_count,
            state.ticks_observed,
            state.thread_count,
            state.runqueue_depth,
            state.current_thread_name,
            state.current_thread_id,
            state.current_thread_role,
            state.context_switches,
            state.bootstrap_thread_id,
            state.idle_thread_id,
        ),
        Err(error) => {
            kprintln!("HXNU: scheduler bootstrap offline reason={}", error.as_str());
            halt();
        }
    }

    let tty_stats = tty::stats();
    let scheduler_stats = sched::stats();
    kprintln!(
        "HXNU: tty stats id={} outputs={} bytes={} lines={}",
        tty_stats.console_id,
        tty_stats.output_count,
        tty_stats.bytes_written,
        tty_stats.lines_written,
    );
    kprintln!(
        "HXNU: scheduler stats threads={} runqueue={} current={}#{} state={} ticks={} switches={} bootstrap-id={} idle-id={}",
        scheduler_stats.thread_count,
        scheduler_stats.runqueue_depth,
        scheduler_stats.current_thread_name,
        scheduler_stats.current_thread_id,
        scheduler_stats.current_thread_state,
        scheduler_stats.total_ticks,
        scheduler_stats.context_switches,
        scheduler_stats.bootstrap_thread_id,
        scheduler_stats.idle_thread_id,
    );

    kprintln!("HXNU: Rust kernel skeleton online");
    sched::idle_loop()
}

const fn selected_self_test() -> Option<SelfTest> {
    if cfg!(feature = "panic-self-test") {
        Some(SelfTest::Panic)
    } else if cfg!(feature = "power-reset-self-test") {
        Some(SelfTest::PowerReset)
    } else if cfg!(feature = "exception-test-page-fault") {
        Some(SelfTest::PageFault)
    } else if cfg!(feature = "exception-test-general-protection") {
        Some(SelfTest::GeneralProtectionFault)
    } else if cfg!(feature = "exception-test-breakpoint") {
        Some(SelfTest::Breakpoint)
    } else {
        Some(SelfTest::Breakpoint)
    }
}

fn halt() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
