#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod acpi;
mod arch;
mod devfs;
mod exec;
mod fb;
mod initrd;
mod init_exec;
#[macro_use]
mod log;
mod limine;
mod mm;
mod panic;
mod power;
mod procfs;
mod sched;
mod serial;
mod smp;
mod syscall;
mod time;
mod tty;
mod uaccess;
mod vfs;

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
                    "HXNU: tty console online id={} outputs={} framebuffer={} vcs={} geometry={}x{}",
                    tty.console_id,
                    tty.output_count,
                    yes_no(tty.framebuffer_output),
                    tty.virtual_console_count,
                    tty.columns,
                    tty.rows,
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
                    "HXNU: tty console online id={} outputs={} framebuffer={} vcs={} geometry={}x{}",
                    tty.console_id,
                    tty.output_count,
                    yes_no(tty.framebuffer_output),
                    tty.virtual_console_count,
                    tty.columns,
                    tty.rows,
                );
            }
        },
        None => {
            let tty = tty::initialize(false);
            kprintln!("HXNU: framebuffer response missing");
            kprintln!(
                "HXNU: tty console online id={} outputs={} framebuffer={} vcs={} geometry={}x{}",
                tty.console_id,
                tty.output_count,
                yes_no(tty.framebuffer_output),
                tty.virtual_console_count,
                tty.columns,
                tty.rows,
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
        "HXNU: cpu local-apic={} x2apic={} tsc-deadline={} invariant-tsc={} nx={} hypervisor={} initial-apic-id={}",
        yes_no(cpu_info.local_apic_supported),
        yes_no(cpu_info.x2apic_supported),
        yes_no(cpu_info.tsc_deadline_supported),
        yes_no(cpu_info.invariant_tsc_supported),
        yes_no(cpu_info.nx_supported),
        yes_no(cpu_info.hypervisor_present),
        cpu_info.initial_apic_id,
    );
    kprintln_style!(
        crate::tty::ConsoleStyle::Muted,
        "HXNU: cpuid vendor={} vendor-id={} max-basic={:#x} max-extended={:#x}",
        cpu_info.vendor.as_str(),
        cpu_info.vendor_str(),
        cpu_info.max_basic_leaf,
        cpu_info.max_extended_leaf,
    );
    if let Some(brand) = cpu_info.brand_str() {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid brand {}",
            brand,
        );
    }
    if let Some(topology) = cpu_info.topology {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid topology leaf={} x2apic-id={} smt-shift={} core-shift={} threads/core={} logical/package={} smt-id={} core-id={} package-id={}",
            topology.leaf_kind.as_str(),
            topology.x2apic_id,
            topology.smt_shift,
            topology.core_shift,
            topology.threads_per_core,
            topology.logical_processors_per_package,
            topology.smt_id,
            topology.core_id,
            topology.package_id,
        );
        for level in topology.levels[..topology.level_count].iter() {
            kprintln_style!(
                crate::tty::ConsoleStyle::Muted,
                "HXNU: cpuid topo level={} type={} shift={} logical={} x2apic-id={}",
                level.level_number,
                level.level_type.as_str(),
                level.shift,
                level.logical_processors,
                level.x2apic_id,
            );
        }
    } else {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: cpuid topology leaf unavailable"
        );
    }
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
                        match smp::initialize(&cpu_info, madt) {
                            Ok(summary) => {
                                kprintln_style!(
                                    crate::tty::ConsoleStyle::Success,
                                    "HXNU: smp topology bsp-apic={} bsp-index={} cpus={} enabled={} online={} aps={} bringup-targets={} x2apic={}",
                                    summary.bsp_apic_id,
                                    summary.current_cpu_index,
                                    summary.total_cpus,
                                    summary.enabled_cpus,
                                    summary.online_cpus,
                                    summary.ap_count,
                                    summary.bringup_targets,
                                    summary.x2apic_cpus,
                                );
                                if let Some(topology) = smp::topology() {
                                    let current = topology.current_cpu();
                                    kprintln!(
                                        "HXNU: smp current cpu{} uid={} apic={} mode={} bsp={} online={}",
                                        current.index,
                                        current.processor_uid,
                                        current.apic_id,
                                        current.apic_mode(),
                                        yes_no(current.is_bsp),
                                        yes_no(current.online),
                                    );
                                    if let Some(target) = topology.first_bringup_target() {
                                        kprintln_style!(
                                            crate::tty::ConsoleStyle::Muted,
                                            "HXNU: smp next ap target cpu{} uid={} apic={} mode={} online-capable={}",
                                            target.index,
                                            target.processor_uid,
                                            target.apic_id,
                                            target.apic_mode(),
                                            yes_no(target.online_capable),
                                        );
                                    }
                                }
                            }
                            Err(error) => kprintln_style!(
                                crate::tty::ConsoleStyle::Error,
                                "HXNU: smp topology offline reason={}",
                                error.as_str()
                            ),
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

    match procfs::initialize(&cpu_info) {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: procfs online directories={} files={} entries={}",
            summary.directory_count,
            summary.file_count,
            summary.entry_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: procfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    match devfs::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: devfs online directories={} nodes={} entries={}",
            summary.directory_count,
            summary.node_count,
            summary.entry_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: devfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    match initrd::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: initrd online modules={} directories={} files={} entries={} bytes={} path={} label={}",
            summary.module_count,
            summary.directory_count,
            summary.file_count,
            summary.entry_count,
            summary.archive_bytes,
            initrd::module_path().unwrap_or("<unknown>"),
            initrd::module_label().unwrap_or("<none>"),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: initrd offline reason={}",
            error.as_str()
        ),
    }
    match vfs::initialize() {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: vfs online mounts={} root-entries={} directories={}",
            summary.mount_count,
            summary.root_entry_count,
            summary.directory_count,
        ),
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: vfs offline reason={}",
                error.as_str()
            );
            halt();
        }
    }
    match vfs::discover_init_executable() {
        Ok(candidate) => kprintln_style!(
            crate::tty::ConsoleStyle::Accent,
            "HXNU: init candidate path={} mount={} format={} size={} executable={}",
            candidate.path,
            candidate.mount.as_str(),
            candidate.format.as_str(),
            candidate.size,
            yes_no(candidate.executable),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init candidate offline reason={}",
            error.as_str()
        ),
    }
    let init_load_prep = vfs::prepare_init_load();
    match &init_load_prep {
        Ok(prep) => kprintln_style!(
            crate::tty::ConsoleStyle::Accent,
            "HXNU: init load-prep path={} mount={} format={} size={} executable={} entry={} machine={} type={} ph={} load={} load-base={} load-offset={} load-file={} load-mem={} load-w={} load-x={} align={} vm-map={} vm-bytes={} vm-zero={} vm-start={} vm-end={} interp={} interp-src={} interp-ok={} interp-arg={}",
            prep.path,
            prep.mount.as_str(),
            prep.format.as_str(),
            prep.size,
            yes_no(prep.executable),
            vfs::format_u64_hex(prep.entry_point),
            vfs::format_u16_hex(prep.machine),
            vfs::format_u16_hex(prep.image_type),
            prep.program_header_count,
            prep.load_segment_count,
            vfs::format_u64_hex(prep.load_base),
            vfs::format_u64_hex(prep.load_offset),
            prep.load_file_bytes,
            prep.load_memory_bytes,
            prep.writable_load_segments,
            prep.executable_load_segments,
            prep.max_alignment,
            prep.vm_map_entries.len(),
            prep.vm_map_total_bytes,
            prep.vm_map_zero_fill_bytes,
            vfs::format_u64_hex(prep.vm_map_start),
            vfs::format_u64_hex(prep.vm_map_end),
            prep.interpreter.as_deref().unwrap_or("<none>"),
            prep.interpreter_source.as_deref().unwrap_or("<none>"),
            yes_no(prep.interpreter_resolved),
            prep.interpreter_argument.as_deref().unwrap_or("<none>"),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init load-prep offline reason={}",
            error.as_str()
        ),
    }
    let init_load_image = vfs::materialize_init_image();
    match &init_load_image {
        Ok(image) => kprintln_style!(
            crate::tty::ConsoleStyle::Accent,
            "HXNU: init load-image path={} mount={} format={} size={} executable={} entry={} machine={} type={} vm-map={} vm-bytes={} vm-zero={} interp={} interp-src={} interp-ok={} interp-arg={}",
            image.path,
            image.mount.as_str(),
            image.format.as_str(),
            image.size,
            yes_no(image.executable),
            vfs::format_u64_hex(image.entry_point),
            vfs::format_u16_hex(image.machine),
            vfs::format_u16_hex(image.image_type),
            image.vm_map_images.len(),
            image.vm_map_total_bytes,
            image.vm_map_zero_fill_bytes,
            image.interpreter.as_deref().unwrap_or("<none>"),
            image.interpreter_source.as_deref().unwrap_or("<none>"),
            yes_no(image.interpreter_resolved),
            image.interpreter_argument.as_deref().unwrap_or("<none>"),
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init load-image offline reason={}",
            error.as_str()
        ),
    }
    let init_handoff = init_exec::activate_init_handoff();
    match &init_handoff {
        Ok(summary) => kprintln_style!(
            crate::tty::ConsoleStyle::Success,
            "HXNU: init handoff armed={} format={} entry={} machine={} type={} vm={}..{} segments={} bytes={} zero={} entry-seg={} entry-off={}",
            yes_no(summary.armed),
            summary.format.as_str(),
            vfs::format_u64_hex(Some(summary.entry_point)),
            vfs::format_u16_hex(Some(summary.machine)),
            vfs::format_u16_hex(Some(summary.image_type)),
            vfs::format_u64_hex(Some(summary.vm_start)),
            vfs::format_u64_hex(Some(summary.vm_end)),
            summary.segment_count,
            summary.total_bytes,
            summary.zero_fill_bytes,
            summary.entry_segment_index,
            summary.entry_segment_map_offset,
        ),
        Err(error) => kprintln_style!(
            crate::tty::ConsoleStyle::Warning,
            "HXNU: init handoff offline reason={}",
            error.as_str()
        ),
    }
    if let Ok(prep) = &init_load_prep {
        if let Some(segment) = prep.vm_map_entries.first() {
            kprintln_style!(
                crate::tty::ConsoleStyle::Muted,
                "HXNU: init vm-map[0] idx={} off={} vaddr={}..{} map={}..{} page-off={} file={} mem={} zero={} align={} perms={}",
                segment.index,
                vfs::format_u64_hex(Some(segment.file_offset)),
                vfs::format_u64_hex(Some(segment.virtual_start)),
                vfs::format_u64_hex(Some(segment.virtual_end)),
                vfs::format_u64_hex(Some(segment.map_start)),
                vfs::format_u64_hex(Some(segment.map_end)),
                segment.page_offset,
                segment.file_bytes,
                segment.memory_bytes,
                segment.zero_fill_bytes,
                segment.alignment,
                vfs::format_rwx(segment.readable, segment.writable, segment.executable),
            );
        }
    }
    if let Ok(image) = &init_load_image {
        if let Some(segment) = image.vm_map_images.first() {
            kprintln_style!(
                crate::tty::ConsoleStyle::Muted,
                "HXNU: init load-image[0] idx={} off={} vaddr={}..{} map={}..{} page-off={} file={} mem={} zero={} bytes={} align={} perms={}",
                segment.index,
                vfs::format_u64_hex(Some(segment.file_offset)),
                vfs::format_u64_hex(Some(segment.virtual_start)),
                vfs::format_u64_hex(Some(segment.virtual_end)),
                vfs::format_u64_hex(Some(segment.map_start)),
                vfs::format_u64_hex(Some(segment.map_end)),
                segment.page_offset,
                segment.file_bytes,
                segment.memory_bytes,
                segment.zero_fill_bytes,
                segment.bytes.len(),
                segment.alignment,
                vfs::format_rwx(segment.readable, segment.writable, segment.executable),
            );
        }
    }
    for console_id in 1..tty::VIRTUAL_CONSOLE_COUNT as u32 {
        let _ = tty::write_to_console(
            console_id,
            crate::tty::ConsoleStyle::Accent,
            "HXNU virtual console ready\n",
        );
        let _ = tty::write_to_console(
            console_id,
            crate::tty::ConsoleStyle::Muted,
            "Framebuffer redraw path prepared for multi-screen TTY\n",
        );
    }
    match tty::switch_active_console(1) {
        Ok(()) => {
            let _ = tty::switch_active_console(0);
            let tty = tty::stats();
            kprintln_style!(
                crate::tty::ConsoleStyle::Success,
                "HXNU: tty virtual consoles online active=tty{} total={} geometry={}x{} switch-smoke=yes",
                tty.console_id,
                tty.virtual_console_count,
                tty.columns,
                tty.rows,
            );
        }
        Err(error) => {
            kprintln_style!(
                crate::tty::ConsoleStyle::Error,
                "HXNU: tty virtual console switch failed reason={}",
                error.as_str()
            );
            halt();
        }
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
            "HXNU: scheduler bootstrap online source={} vector={:#04x} divide={} initial-count={} ticks={} threads={} runqueue={} current={}#{} pid={} ppid={} role={} switches={} bootstrap-id={} idle-id={}",
            state.source,
            state.vector,
            state.divide_value,
            state.initial_count,
            state.ticks_observed,
            state.thread_count,
            state.runqueue_depth,
            state.current_thread_name,
            state.current_thread_id,
            state.current_process_id,
            state.current_parent_process_id,
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
        "HXNU: scheduler stats threads={} runqueue={} current={}#{} pid={} ppid={} state={} ticks={} switches={} bootstrap-id={} idle-id={}",
        scheduler_stats.thread_count,
        scheduler_stats.runqueue_depth,
        scheduler_stats.current_thread_name,
        scheduler_stats.current_thread_id,
        scheduler_stats.current_process_id,
        scheduler_stats.current_parent_process_id,
        scheduler_stats.current_thread_state,
        scheduler_stats.total_ticks,
        scheduler_stats.context_switches,
        scheduler_stats.bootstrap_thread_id,
        scheduler_stats.idle_thread_id,
    );
    let linux_probe = syscall::run_linux_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} write={} openat={} mmap={} mprotect={} munmap={} brk={} brk_set={} brk_restore={} nanosleep={} gettimeofday={} wall={}.{:06} getrandom={} random={:#x} rt_sigaction={} rt_sigprocmask={} sigmask={:#x} old_handler={:#x} pread64={} pwrite64={} readv={} writev={} wait4={} setpgid={} getpgid={} setsid={} getsid={} getrlimit={} setrlimit={} prlimit64={} prctl_set_name={} prctl_get_name={} prctl_set_dumpable={} prctl_get_dumpable={} set_robust_list={} get_robust_list={} rseq_register={} rseq_unregister={} arch_prctl_set_fs={} arch_prctl_get_fs={} arch_prctl_set_gs={} arch_prctl_get_gs={} futex_wait={} futex_wake={} pipe2={} poll={} ppoll={} ioctl={} access={} newfstatat={} faccessat={} faccessat2={} readlinkat={} dup={} dup2={} dup3={} fcntl_getfd={} fcntl_getfl={} getcwd={} chdir={} fchdir={} read={} fstat={} getdents64={} lseek={} close={} getpid={} getppid={} gettid={} umask={} umask_restore={} getuid={} getgid={} geteuid={} getegid={} set_tid_address={} clear_tid={} sched_yield={} clock_gettime={} monotonic={}.{:09} uname={} machine={} exit-captured={} exit-status={}",
        syscall::SyscallAbi::LinuxBootstrap.as_str(),
        linux_probe.write_result,
        linux_probe.openat_result,
        linux_probe.mmap_result,
        linux_probe.mprotect_result,
        linux_probe.munmap_result,
        linux_probe.brk_result,
        linux_probe.brk_set_result,
        linux_probe.brk_restore_result,
        linux_probe.nanosleep_result,
        linux_probe.gettimeofday_result,
        linux_probe.gettimeofday_seconds,
        linux_probe.gettimeofday_microseconds,
        linux_probe.getrandom_result,
        linux_probe.getrandom_sample,
        linux_probe.rt_sigaction_result,
        linux_probe.rt_sigprocmask_result,
        linux_probe.rt_sigmask_snapshot,
        linux_probe.rt_sigold_handler,
        linux_probe.pread64_result,
        linux_probe.pwrite64_result,
        linux_probe.readv_result,
        linux_probe.writev_result,
        linux_probe.wait4_result,
        linux_probe.setpgid_result,
        linux_probe.getpgid_result,
        linux_probe.setsid_result,
        linux_probe.getsid_result,
        linux_probe.getrlimit_result,
        linux_probe.setrlimit_result,
        linux_probe.prlimit64_result,
        linux_probe.prctl_set_name_result,
        linux_probe.prctl_get_name_result,
        linux_probe.prctl_set_dumpable_result,
        linux_probe.prctl_get_dumpable_result,
        linux_probe.set_robust_list_result,
        linux_probe.get_robust_list_result,
        linux_probe.rseq_register_result,
        linux_probe.rseq_unregister_result,
        linux_probe.arch_prctl_set_fs_result,
        linux_probe.arch_prctl_get_fs_result,
        linux_probe.arch_prctl_set_gs_result,
        linux_probe.arch_prctl_get_gs_result,
        linux_probe.futex_wait_result,
        linux_probe.futex_wake_result,
        linux_probe.pipe2_result,
        linux_probe.poll_result,
        linux_probe.ppoll_result,
        linux_probe.ioctl_result,
        linux_probe.access_result,
        linux_probe.newfstatat_result,
        linux_probe.faccessat_result,
        linux_probe.faccessat2_result,
        linux_probe.readlinkat_result,
        linux_probe.dup_result,
        linux_probe.dup2_result,
        linux_probe.dup3_result,
        linux_probe.fcntl_getfd_result,
        linux_probe.fcntl_getfl_result,
        linux_probe.getcwd_result,
        linux_probe.chdir_result,
        linux_probe.fchdir_result,
        linux_probe.read_result,
        linux_probe.fstat_result,
        linux_probe.getdents64_result,
        linux_probe.lseek_result,
        linux_probe.close_result,
        linux_probe.getpid_result,
        linux_probe.getppid_result,
        linux_probe.gettid_result,
        linux_probe.umask_result,
        linux_probe.umask_restore_result,
        linux_probe.getuid_result,
        linux_probe.getgid_result,
        linux_probe.geteuid_result,
        linux_probe.getegid_result,
        linux_probe.set_tid_address_result,
        linux_probe.clear_tid_snapshot,
        linux_probe.sched_yield_result,
        linux_probe.clock_gettime_result,
        linux_probe.clock_seconds,
        linux_probe.clock_nanoseconds,
        linux_probe.uname_result,
        linux_probe.machine_str(),
        yes_no(linux_probe.exit_group_captured),
        linux_probe.exit_group_status,
    );
    let ghost_probe = syscall::run_ghost_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} write={} open={} mmap={} mprotect={} munmap={} brk={} brk_set={} brk_restore={} nanosleep={} gettimeofday={} wall={}.{:06} getrandom={} random={:#x} rt_sigaction={} rt_sigprocmask={} sigmask={:#x} old_handler={:#x} pread64={} pwrite64={} readv={} writev={} wait4={} setpgid={} getpgid={} setsid={} getsid={} getrlimit={} setrlimit={} prlimit64={} prctl_set_name={} prctl_get_name={} prctl_set_dumpable={} prctl_get_dumpable={} set_robust_list={} get_robust_list={} rseq_register={} rseq_unregister={} arch_prctl_set_fs={} arch_prctl_get_fs={} arch_prctl_set_gs={} arch_prctl_get_gs={} futex_wait={} futex_wake={} pipe2={} poll={} ppoll={} ioctl={} access={} stat={} readlink={} dup={} dup2={} dup3={} fcntl_getfd={} fcntl_getfl={} getcwd={} chdir={} fchdir={} read={} fstat={} getdents={} seek={} close={} getpid={} getppid={} gettid={} umask={} umask_restore={} getuid={} getgid={} geteuid={} getegid={} set_tid_address={} clear_tid={} yield={} uptime-ns={} uname={} machine={} exit-captured={} exit-status={}",
        syscall::SyscallAbi::GhostBootstrap.as_str(),
        ghost_probe.write_result,
        ghost_probe.open_result,
        ghost_probe.mmap_result,
        ghost_probe.mprotect_result,
        ghost_probe.munmap_result,
        ghost_probe.brk_result,
        ghost_probe.brk_set_result,
        ghost_probe.brk_restore_result,
        ghost_probe.nanosleep_result,
        ghost_probe.gettimeofday_result,
        ghost_probe.gettimeofday_seconds,
        ghost_probe.gettimeofday_microseconds,
        ghost_probe.getrandom_result,
        ghost_probe.getrandom_sample,
        ghost_probe.rt_sigaction_result,
        ghost_probe.rt_sigprocmask_result,
        ghost_probe.rt_sigmask_snapshot,
        ghost_probe.rt_sigold_handler,
        ghost_probe.pread64_result,
        ghost_probe.pwrite64_result,
        ghost_probe.readv_result,
        ghost_probe.writev_result,
        ghost_probe.wait4_result,
        ghost_probe.setpgid_result,
        ghost_probe.getpgid_result,
        ghost_probe.setsid_result,
        ghost_probe.getsid_result,
        ghost_probe.getrlimit_result,
        ghost_probe.setrlimit_result,
        ghost_probe.prlimit64_result,
        ghost_probe.prctl_set_name_result,
        ghost_probe.prctl_get_name_result,
        ghost_probe.prctl_set_dumpable_result,
        ghost_probe.prctl_get_dumpable_result,
        ghost_probe.set_robust_list_result,
        ghost_probe.get_robust_list_result,
        ghost_probe.rseq_register_result,
        ghost_probe.rseq_unregister_result,
        ghost_probe.arch_prctl_set_fs_result,
        ghost_probe.arch_prctl_get_fs_result,
        ghost_probe.arch_prctl_set_gs_result,
        ghost_probe.arch_prctl_get_gs_result,
        ghost_probe.futex_wait_result,
        ghost_probe.futex_wake_result,
        ghost_probe.pipe2_result,
        ghost_probe.poll_result,
        ghost_probe.ppoll_result,
        ghost_probe.ioctl_result,
        ghost_probe.access_result,
        ghost_probe.stat_result,
        ghost_probe.readlink_result,
        ghost_probe.dup_result,
        ghost_probe.dup2_result,
        ghost_probe.dup3_result,
        ghost_probe.fcntl_getfd_result,
        ghost_probe.fcntl_getfl_result,
        ghost_probe.getcwd_result,
        ghost_probe.chdir_result,
        ghost_probe.fchdir_result,
        ghost_probe.read_result,
        ghost_probe.fstat_result,
        ghost_probe.getdents_result,
        ghost_probe.seek_result,
        ghost_probe.close_result,
        ghost_probe.getpid_result,
        ghost_probe.getppid_result,
        ghost_probe.gettid_result,
        ghost_probe.umask_result,
        ghost_probe.umask_restore_result,
        ghost_probe.getuid_result,
        ghost_probe.getgid_result,
        ghost_probe.geteuid_result,
        ghost_probe.getegid_result,
        ghost_probe.set_tid_address_result,
        ghost_probe.clear_tid_snapshot,
        ghost_probe.yield_result,
        ghost_probe.uptime_result,
        ghost_probe.uname_result,
        ghost_probe.machine_str(),
        yes_no(ghost_probe.exit_group_captured),
        ghost_probe.exit_group_status,
    );
    let hxnu_probe = syscall::run_hxnu_bootstrap_probe();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall bootstrap abi={} log_write={} open={} mmap={} mprotect={} munmap={} brk={} brk_set={} brk_restore={} nanosleep={} gettimeofday={} wall={}.{:06} getrandom={} random={:#x} rt_sigaction={} rt_sigprocmask={} sigmask={:#x} old_handler={:#x} pread64={} pwrite64={} readv={} writev={} wait4={} setpgid={} getpgid={} setsid={} getsid={} getrlimit={} setrlimit={} prlimit64={} prctl_set_name={} prctl_get_name={} prctl_set_dumpable={} prctl_get_dumpable={} set_robust_list={} get_robust_list={} rseq_register={} rseq_unregister={} arch_prctl_set_fs={} arch_prctl_get_fs={} arch_prctl_set_gs={} arch_prctl_get_gs={} futex_wait={} futex_wake={} pipe2={} poll={} ppoll={} ioctl={} access={} stat={} readlink={} dup={} dup2={} dup3={} fcntl_getfd={} fcntl_getfl={} getcwd={} chdir={} fchdir={} read={} fstat={} getdents={} seek={} close={} process_self={} process_parent={} thread_self={} umask={} umask_restore={} getuid={} getgid={} geteuid={} getegid={} set_tid_address={} clear_tid={} sched_yield={} uptime-ns={} abi-version={:#x} exit-captured={} exit-status={}",
        syscall::SyscallAbi::HxnuNativeBootstrap.as_str(),
        hxnu_probe.write_result,
        hxnu_probe.open_result,
        hxnu_probe.mmap_result,
        hxnu_probe.mprotect_result,
        hxnu_probe.munmap_result,
        hxnu_probe.brk_result,
        hxnu_probe.brk_set_result,
        hxnu_probe.brk_restore_result,
        hxnu_probe.nanosleep_result,
        hxnu_probe.gettimeofday_result,
        hxnu_probe.gettimeofday_seconds,
        hxnu_probe.gettimeofday_microseconds,
        hxnu_probe.getrandom_result,
        hxnu_probe.getrandom_sample,
        hxnu_probe.rt_sigaction_result,
        hxnu_probe.rt_sigprocmask_result,
        hxnu_probe.rt_sigmask_snapshot,
        hxnu_probe.rt_sigold_handler,
        hxnu_probe.pread64_result,
        hxnu_probe.pwrite64_result,
        hxnu_probe.readv_result,
        hxnu_probe.writev_result,
        hxnu_probe.wait4_result,
        hxnu_probe.setpgid_result,
        hxnu_probe.getpgid_result,
        hxnu_probe.setsid_result,
        hxnu_probe.getsid_result,
        hxnu_probe.getrlimit_result,
        hxnu_probe.setrlimit_result,
        hxnu_probe.prlimit64_result,
        hxnu_probe.prctl_set_name_result,
        hxnu_probe.prctl_get_name_result,
        hxnu_probe.prctl_set_dumpable_result,
        hxnu_probe.prctl_get_dumpable_result,
        hxnu_probe.set_robust_list_result,
        hxnu_probe.get_robust_list_result,
        hxnu_probe.rseq_register_result,
        hxnu_probe.rseq_unregister_result,
        hxnu_probe.arch_prctl_set_fs_result,
        hxnu_probe.arch_prctl_get_fs_result,
        hxnu_probe.arch_prctl_set_gs_result,
        hxnu_probe.arch_prctl_get_gs_result,
        hxnu_probe.futex_wait_result,
        hxnu_probe.futex_wake_result,
        hxnu_probe.pipe2_result,
        hxnu_probe.poll_result,
        hxnu_probe.ppoll_result,
        hxnu_probe.ioctl_result,
        hxnu_probe.access_result,
        hxnu_probe.stat_result,
        hxnu_probe.readlink_result,
        hxnu_probe.dup_result,
        hxnu_probe.dup2_result,
        hxnu_probe.dup3_result,
        hxnu_probe.fcntl_getfd_result,
        hxnu_probe.fcntl_getfl_result,
        hxnu_probe.getcwd_result,
        hxnu_probe.chdir_result,
        hxnu_probe.fchdir_result,
        hxnu_probe.read_result,
        hxnu_probe.fstat_result,
        hxnu_probe.getdents_result,
        hxnu_probe.seek_result,
        hxnu_probe.close_result,
        hxnu_probe.process_self_result,
        hxnu_probe.process_parent_result,
        hxnu_probe.thread_self_result,
        hxnu_probe.umask_result,
        hxnu_probe.umask_restore_result,
        hxnu_probe.getuid_result,
        hxnu_probe.getgid_result,
        hxnu_probe.geteuid_result,
        hxnu_probe.getegid_result,
        hxnu_probe.set_tid_address_result,
        hxnu_probe.clear_tid_snapshot,
        hxnu_probe.sched_yield_result,
        hxnu_probe.uptime_result,
        hxnu_probe.abi_version_result,
        yes_no(hxnu_probe.exit_group_captured),
        hxnu_probe.exit_group_status,
    );
    let syscall_self_test = arch::x86_64::run_syscall_self_test();
    kprintln_style!(
        crate::tty::ConsoleStyle::Success,
        "HXNU: syscall entry self-test int=0x80 linux_write={} linux_openat={} linux_read={} linux_close={} linux_getpid={} ghost_open={} ghost_read={} ghost_close={} ghost_gettid={} hxnu_open={} hxnu_read={} hxnu_close={} hxnu_abi_version={:#x}",
        syscall_self_test.linux_write_result,
        syscall_self_test.linux_openat_result,
        syscall_self_test.linux_read_result,
        syscall_self_test.linux_close_result,
        syscall_self_test.linux_getpid_result,
        syscall_self_test.ghost_open_result,
        syscall_self_test.ghost_read_result,
        syscall_self_test.ghost_close_result,
        syscall_self_test.ghost_gettid_result,
        syscall_self_test.hxnu_open_result,
        syscall_self_test.hxnu_read_result,
        syscall_self_test.hxnu_close_result,
        syscall_self_test.hxnu_abi_version_result,
    );
    if let Some(root) = vfs::preview("/", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: vfs preview root={}",
            root,
        );
    }
    if let Some(version) = vfs::preview("/proc/version", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview version={}",
            version,
        );
    }
    if let Some(uptime) = vfs::preview("/proc/uptime", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview uptime={}",
            uptime,
        );
    }
    if let Some(schedstat) = vfs::preview("/proc/schedstat", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: procfs preview schedstat={}",
            schedstat,
        );
    }
    if let Some(devlist) = vfs::preview("/dev", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: devfs preview root={}",
            devlist,
        );
    }
    if let Some(initrd_root) = vfs::preview("/initrd", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: initrd preview root={}",
            initrd_root,
        );
    }
    if let Some(init) = vfs::preview("/initrd/init", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: initrd preview init={}",
            init,
        );
    }
    if let Some(console) = vfs::preview("/dev/console", 80) {
        kprintln_style!(
            crate::tty::ConsoleStyle::Muted,
            "HXNU: devfs preview console={}",
            console,
        );
    }

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
