use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::arch;
use crate::mm;
use crate::sched;
use crate::smp;
use crate::time;

const PROCFS_DIRECTORIES: [&str; 2] = ["/", "/proc"];
const PROCFS_FILES: [&str; 6] = [
    "/proc/version",
    "/proc/uptime",
    "/proc/meminfo",
    "/proc/cpuinfo",
    "/proc/schedstat",
    "/proc/topology",
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
