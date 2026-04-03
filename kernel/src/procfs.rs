use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::arch;
use crate::block;
use crate::fat;
use crate::init_exec;
use crate::mm;
use crate::sched;
use crate::smp;
use crate::syscall;
use crate::time;

const PROCFS_DIRECTORIES: [&str; 2] = ["/", "/proc"];
const PROCFS_FILES: [&str; 11] = [
    "/proc/version",
    "/proc/uptime",
    "/proc/meminfo",
    "/proc/cpuinfo",
    "/proc/schedstat",
    "/proc/topology",
    "/proc/initexec",
    "/proc/exec",
    "/proc/compress",
    "/proc/block",
    "/proc/fat",
];

struct GlobalProcfs(UnsafeCell<Option<ProcfsState>>);

unsafe impl Sync for GlobalProcfs {}

impl GlobalProcfs {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<ProcfsState> {
        self.0.get()
    }
}

static PROCFS: GlobalProcfs = GlobalProcfs::new();

#[derive(Clone)]
struct ProcfsState {
    boot_cpu: arch::x86_64::CpuInfo,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ProcfsNodeKind {
    Directory,
    File,
}

#[derive(Copy, Clone)]
pub struct ProcfsSummary {
    pub directory_count: usize,
    pub file_count: usize,
    pub entry_count: usize,
}

#[derive(Copy, Clone)]
pub enum ProcfsError {
    AlreadyInitialized,
}

impl ProcfsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "procfs is already initialized",
        }
    }
}

pub fn initialize(cpu_info: &arch::x86_64::CpuInfo) -> Result<ProcfsSummary, ProcfsError> {
    let slot = unsafe { &mut *PROCFS.get() };
    if slot.is_some() {
        return Err(ProcfsError::AlreadyInitialized);
    }

    *slot = Some(ProcfsState {
        boot_cpu: cpu_info.clone(),
    });

    Ok(summary())
}

pub fn summary() -> ProcfsSummary {
    ProcfsSummary {
        directory_count: PROCFS_DIRECTORIES.len(),
        file_count: PROCFS_FILES.len(),
        entry_count: PROCFS_DIRECTORIES.len() + PROCFS_FILES.len(),
    }
}

pub fn node_kind(path: &str) -> Option<ProcfsNodeKind> {
    match path {
        "/proc" | "/proc/" => Some(ProcfsNodeKind::Directory),
        _ if PROCFS_FILES.iter().any(|file| *file == path) => Some(ProcfsNodeKind::File),
        _ => None,
    }
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*PROCFS.get()).as_ref()? };
    match path {
        "/proc" | "/proc/" => Some(render_root()),
        "/proc/version" => Some(render_version()),
        "/proc/uptime" => Some(render_uptime()),
        "/proc/meminfo" => Some(render_meminfo()),
        "/proc/cpuinfo" => Some(render_cpuinfo(state)),
        "/proc/schedstat" => Some(render_schedstat()),
        "/proc/topology" => Some(render_topology(state)),
        "/proc/initexec" => Some(init_exec::render_status()),
        "/proc/exec" => Some(syscall::render_exec_status()),
        "/proc/compress" => Some(render_compress()),
        "/proc/block" => Some(render_block()),
        "/proc/fat" => Some(render_fat()),
        _ => None,
    }
}

fn render_root() -> String {
    let mut text = String::new();
    for file in PROCFS_FILES {
        let _ = writeln!(text, "{}", file.trim_start_matches("/proc/"));
    }
    text
}

fn render_version() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "HXNU 2605 x86_64");
    text
}

fn render_uptime() -> String {
    let mut text = String::new();
    let uptime_ns = time::uptime_nanoseconds();
    let seconds = uptime_ns / 1_000_000_000;
    let nanos = uptime_ns % 1_000_000_000;
    let _ = writeln!(text, "{}.{:09} {}.{:09}", seconds, nanos, seconds, nanos);
    text
}

fn render_meminfo() -> String {
    let mut text = String::new();
    let frame = mm::frame::stats();
    let heap = mm::heap::stats();
    let frame_allocated_bytes = frame.allocated_frames.saturating_mul(mm::frame::PAGE_SIZE);
    let frame_free_bytes = frame.allocatable_bytes.saturating_sub(frame_allocated_bytes);

    let _ = writeln!(text, "MemTotal:       {} kB", frame.total_bytes / 1024);
    let _ = writeln!(text, "MemAllocatable: {} kB", frame.allocatable_bytes / 1024);
    let _ = writeln!(text, "MemFree:        {} kB", frame_free_bytes / 1024);
    let _ = writeln!(text, "FrameRegions:   {}", frame.usable_regions);
    let _ = writeln!(text, "FrameAllocated: {}", frame.allocated_frames);
    let _ = writeln!(text, "HeapTotal:      {} kB", heap.size_bytes / 1024);
    let _ = writeln!(text, "HeapUsed:       {} kB", heap.used_bytes / 1024);
    let _ = writeln!(text, "HeapAllocs:     {}", heap.allocation_count);
    text
}

fn render_cpuinfo(state: &ProcfsState) -> String {
    let mut text = String::new();
    let flags = cpu_flags(&state.boot_cpu);
    let topology = smp::topology();

    if let Some(topology) = topology {
        for cpu in &topology.cpus {
            let _ = writeln!(text, "processor\t: {}", cpu.index);
            let _ = writeln!(text, "vendor_id\t: {}", state.boot_cpu.vendor_str());
            if let Some(brand) = state.boot_cpu.brand_str() {
                let _ = writeln!(text, "model name\t: {}", brand);
            }
            let _ = writeln!(text, "apicid\t\t: {}", cpu.apic_id);
            let _ = writeln!(text, "cpu family\t: {}", state.boot_cpu.vendor.as_str());
            let _ = writeln!(text, "topology\t: {}", cpu.apic_mode());
            let _ = writeln!(text, "bsp\t\t: {}", yes_no(cpu.is_bsp));
            let _ = writeln!(text, "online\t\t: {}", yes_no(cpu.online));
            let _ = writeln!(text, "flags\t\t: {}", flags);
            let _ = writeln!(text);
        }
    } else {
        let _ = writeln!(text, "processor\t: 0");
        let _ = writeln!(text, "vendor_id\t: {}", state.boot_cpu.vendor_str());
        if let Some(brand) = state.boot_cpu.brand_str() {
            let _ = writeln!(text, "model name\t: {}", brand);
        }
        let _ = writeln!(text, "apicid\t\t: {}", state.boot_cpu.initial_apic_id);
        let _ = writeln!(text, "flags\t\t: {}", flags);
    }

    text
}

fn render_schedstat() -> String {
    let mut text = String::new();
    let stats = sched::stats();
    let _ = writeln!(text, "threads {}", stats.thread_count);
    let _ = writeln!(text, "runqueue {}", stats.runqueue_depth);
    let _ = writeln!(text, "current_id {}", stats.current_thread_id);
    let _ = writeln!(text, "current_pid {}", stats.current_process_id);
    let _ = writeln!(text, "current_ppid {}", stats.current_parent_process_id);
    let _ = writeln!(text, "current_name {}", stats.current_thread_name);
    let _ = writeln!(text, "current_role {}", stats.current_thread_role);
    let _ = writeln!(text, "current_state {}", stats.current_thread_state);
    let _ = writeln!(text, "ticks {}", stats.total_ticks);
    let _ = writeln!(text, "switches {}", stats.context_switches);
    let _ = writeln!(text, "bootstrap_id {}", stats.bootstrap_thread_id);
    let _ = writeln!(text, "idle_id {}", stats.idle_thread_id);
    text
}

fn render_topology(state: &ProcfsState) -> String {
    let mut text = String::new();
    if let Some(topology) = smp::topology() {
        let summary = topology.summary();
        let _ = writeln!(text, "bsp_apic_id {}", summary.bsp_apic_id);
        let _ = writeln!(text, "bsp_index {}", summary.current_cpu_index);
        let _ = writeln!(text, "cpus {}", summary.total_cpus);
        let _ = writeln!(text, "enabled {}", summary.enabled_cpus);
        let _ = writeln!(text, "online {}", summary.online_cpus);
        let _ = writeln!(text, "aps {}", summary.ap_count);
        let _ = writeln!(text, "bringup_targets {}", summary.bringup_targets);
        let _ = writeln!(text, "x2apic {}", summary.x2apic_cpus);
        if let Some(cpuid_topology) = state.boot_cpu.topology {
            let _ = writeln!(text, "cpuid_leaf {}", cpuid_topology.leaf_kind.as_str());
            let _ = writeln!(text, "cpuid_package {}", cpuid_topology.package_id);
            let _ = writeln!(text, "cpuid_core {}", cpuid_topology.core_id);
            let _ = writeln!(text, "cpuid_smt {}", cpuid_topology.smt_id);
        }
        for cpu in &topology.cpus {
            let _ = writeln!(
                text,
                "cpu{} uid={} apic={} mode={} bsp={} online={} enabled={}",
                cpu.index,
                cpu.processor_uid,
                cpu.apic_id,
                cpu.apic_mode(),
                yes_no(cpu.is_bsp),
                yes_no(cpu.online),
                yes_no(cpu.enabled),
            );
        }
    } else {
        let _ = writeln!(text, "bsp_apic_id {}", state.boot_cpu.initial_apic_id);
        let _ = writeln!(text, "cpus 1");
        let _ = writeln!(text, "online 1");
    }
    text
}

fn render_compress() -> String {
    let mut text = String::new();
    let runtime = mm::compress::summary();
    let codec = mm::compress::stats();
    let store = mm::compress::store::stats();
    let pager = mm::pager::stats();

    let _ = writeln!(text, "runtime_initialized {}", yes_no(mm::compress::is_initialized()));
    let _ = writeln!(text, "runtime_backend {}", runtime.backend);
    let _ = writeln!(text, "runtime_profile {}", runtime.profile);
    let _ = writeln!(text, "runtime_profile_version {}", runtime.profile_version);
    let _ = writeln!(text, "runtime_page_bytes {}", runtime.page_bytes);
    let _ = writeln!(text, "runtime_header_bytes {}", runtime.encoded_header_bytes);
    let _ = writeln!(text, "runtime_max_encoded_page_bytes {}", runtime.max_encoded_page_bytes);
    let _ = writeln!(text, "runtime_dictionary_entries {}", runtime.static_dictionary_entries);
    let _ = writeln!(text, "runtime_pattern_entries {}", runtime.static_pattern_entries);

    let _ = writeln!(text, "codec_encoded_pages {}", codec.encoded_pages);
    let _ = writeln!(text, "codec_decoded_pages {}", codec.decoded_pages);
    let _ = writeln!(text, "codec_zero_pages {}", codec.zero_pages);
    let _ = writeln!(text, "codec_same_pages {}", codec.same_pages);
    let _ = writeln!(text, "codec_sxrc_pages {}", codec.sxrc_pages);
    let _ = writeln!(text, "codec_raw_pages {}", codec.raw_pages);
    let _ = writeln!(text, "codec_raw_fallback_pages {}", codec.fallback_raw_pages);
    let _ = writeln!(text, "codec_encode_failures {}", codec.encode_failures);
    let _ = writeln!(text, "codec_decode_failures {}", codec.decode_failures);

    let _ = writeln!(text, "store_initialized {}", yes_no(mm::compress::store::is_initialized()));
    let _ = writeln!(text, "store_capacity_pages {}", store.capacity_pages);
    let _ = writeln!(text, "store_capacity_bytes {}", store.capacity_bytes);
    let _ = writeln!(text, "store_stored_pages {}", store.stored_pages);
    let _ = writeln!(text, "store_stored_zero_pages {}", store.stored_zero_pages);
    let _ = writeln!(text, "store_stored_same_pages {}", store.stored_same_pages);
    let _ = writeln!(text, "store_stored_sxrc_pages {}", store.stored_sxrc_pages);
    let _ = writeln!(text, "store_stored_raw_pages {}", store.stored_raw_pages);
    let _ = writeln!(text, "store_current_encoded_bytes {}", store.current_encoded_bytes);
    let _ = writeln!(text, "store_total_input_bytes {}", store.total_input_bytes);
    let _ = writeln!(text, "store_total_encoded_bytes {}", store.total_encoded_bytes);
    let _ = writeln!(text, "store_requests {}", store.store_requests);
    let _ = writeln!(text, "store_successes {}", store.store_successes);
    let _ = writeln!(text, "store_load_requests {}", store.load_requests);
    let _ = writeln!(text, "store_load_successes {}", store.load_successes);
    let _ = writeln!(text, "store_load_misses {}", store.load_misses);
    let _ = writeln!(text, "store_replacements {}", store.replacements);
    let _ = writeln!(text, "store_evictions {}", store.evictions);
    let _ = writeln!(text, "store_encode_failures {}", store.encode_failures);
    let _ = writeln!(text, "store_decode_failures {}", store.decode_failures);

    let _ = writeln!(text, "pager_initialized {}", yes_no(mm::pager::is_initialized()));
    let _ = writeln!(text, "pager_reclaim_requests {}", pager.reclaim_requests);
    let _ = writeln!(text, "pager_reclaim_successes {}", pager.reclaim_successes);
    let _ = writeln!(text, "pager_reclaim_failures {}", pager.reclaim_failures);
    let _ = writeln!(text, "pager_restore_requests {}", pager.restore_requests);
    let _ = writeln!(text, "pager_restore_successes {}", pager.restore_successes);
    let _ = writeln!(text, "pager_restore_failures {}", pager.restore_failures);
    let _ = writeln!(text, "pager_restore_misses {}", pager.restore_misses);
    let _ = writeln!(text, "pager_verify_failures {}", pager.verify_failures);
    let _ = writeln!(text, "pager_reclaimed_zero_pages {}", pager.reclaimed_zero_pages);
    let _ = writeln!(text, "pager_reclaimed_same_pages {}", pager.reclaimed_same_pages);
    let _ = writeln!(text, "pager_reclaimed_sxrc_pages {}", pager.reclaimed_sxrc_pages);
    let _ = writeln!(text, "pager_reclaimed_raw_pages {}", pager.reclaimed_raw_pages);
    let _ = writeln!(text, "pager_restored_zero_pages {}", pager.restored_zero_pages);
    let _ = writeln!(text, "pager_restored_same_pages {}", pager.restored_same_pages);
    let _ = writeln!(text, "pager_restored_sxrc_pages {}", pager.restored_sxrc_pages);
    let _ = writeln!(text, "pager_restored_raw_pages {}", pager.restored_raw_pages);
    let _ = writeln!(text, "pager_smoke_runs {}", pager.smoke_runs);
    let _ = writeln!(text, "pager_smoke_successes {}", pager.smoke_successes);
    text
}

fn render_block() -> String {
    let mut text = String::new();
    let summary = block::summary();
    let stats = block::stats();

    let _ = writeln!(text, "initialized {}", yes_no(block::is_initialized()));
    let _ = writeln!(text, "driver_count {}", summary.driver_count);
    let _ = writeln!(text, "device_count {}", summary.device_count);
    let _ = writeln!(text, "partition_count {}", summary.partition_count);
    let _ = writeln!(text, "total_bytes {}", summary.total_bytes);
    let _ = writeln!(text, "mbr_devices {}", summary.mbr_device_count);
    let _ = writeln!(text, "gpt_devices {}", summary.gpt_device_count);
    let _ = writeln!(text, "read_requests {}", stats.read_requests);
    let _ = writeln!(text, "read_sectors {}", stats.read_sectors);
    let _ = writeln!(text, "read_bytes {}", stats.read_bytes);
    let _ = writeln!(text, "read_failures {}", stats.read_failures);

    let mut index = 0usize;
    while index < block::device_count() {
        if let Some(device) = block::device(index) {
            let _ = writeln!(
                text,
                "device{} id={} name={} kind={} ro={} sectors={} sector-bytes={} size={}",
                index,
                device.id,
                device.name,
                device.kind.as_str(),
                yes_no(device.read_only),
                device.sector_count,
                device.sector_size,
                device.size_bytes,
            );
            let _ = writeln!(text, "device{} driver={}", index, device.driver_name);
        }
        index += 1;
    }

    let mut part_index = 0usize;
    while part_index < block::partition_count() {
        if let Some(partition) = block::partition(part_index) {
            match partition.table_kind {
                block::PartitionTableKind::Mbr => {
                    let _ = writeln!(
                        text,
                        "partition{} id={} dev={} table={} mbr-index={} type={:#04x} bootable={} lba={} sectors={}",
                        part_index,
                        partition.id,
                        partition.device_id,
                        partition.table_kind.as_str(),
                        partition.mbr_index,
                        partition.partition_type,
                        yes_no(partition.bootable),
                        partition.start_lba,
                        partition.sector_count,
                    );
                }
                block::PartitionTableKind::Gpt => {
                    let _ = writeln!(
                        text,
                        "partition{} id={} dev={} table={} gpt-index={} type-guid={} part-guid={} lba={} sectors={}",
                        part_index,
                        partition.id,
                        partition.device_id,
                        partition.table_kind.as_str(),
                        partition.gpt_index,
                        format_guid(&partition.gpt_type_guid),
                        format_guid(&partition.gpt_partition_guid),
                        partition.start_lba,
                        partition.sector_count,
                    );
                }
            }
        }
        part_index += 1;
    }

    text
}

fn render_fat() -> String {
    let mut text = String::new();
    let summary = fat::summary();

    let _ = writeln!(text, "initialized {}", yes_no(fat::is_initialized()));
    let _ = writeln!(text, "mounted {}", yes_no(summary.mounted));
    let _ = writeln!(text, "partition_id {:?}", summary.partition_id);
    let _ = writeln!(text, "device_id {:?}", summary.device_id);
    let _ = writeln!(
        text,
        "partition_table {}",
        summary.partition_table.map_or("<none>", |kind| kind.as_str())
    );
    let _ = writeln!(
        text,
        "fat_type {}",
        summary.fat_type.map_or("<none>", |kind| kind.as_str())
    );
    let _ = writeln!(text, "root_entry_count {}", summary.root_entry_count);
    let _ = writeln!(text, "directory_count {}", summary.directory_count);
    text
}

fn format_guid(guid: &[u8; 16]) -> String {
    let mut text = String::new();
    for (index, byte) in guid.iter().copied().enumerate() {
        if index == 4 || index == 6 || index == 8 || index == 10 {
            text.push('-');
        }
        append_hex_byte(&mut text, byte);
    }
    text
}

fn append_hex_byte(text: &mut String, byte: u8) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    text.push(HEX[(byte >> 4) as usize] as char);
    text.push(HEX[(byte & 0x0f) as usize] as char);
}

fn cpu_flags(cpu: &arch::x86_64::CpuInfo) -> String {
    let mut text = String::new();
    append_flag(&mut text, "apic");
    if cpu.x2apic_supported {
        append_flag(&mut text, "x2apic");
    }
    if cpu.tsc_deadline_supported {
        append_flag(&mut text, "tsc_deadline");
    }
    if cpu.invariant_tsc_supported {
        append_flag(&mut text, "invariant_tsc");
    }
    if cpu.nx_supported {
        append_flag(&mut text, "nx");
    }
    if cpu.hypervisor_present {
        append_flag(&mut text, "hypervisor");
    }
    text
}

fn append_flag(text: &mut String, flag: &str) {
    if !text.is_empty() {
        text.push(' ');
    }
    text.push_str(flag);
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
