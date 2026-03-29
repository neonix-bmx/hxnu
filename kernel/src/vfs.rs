use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::devfs;
use crate::devfs::DevfsNodeKind;
use crate::exec;
use crate::initrd;
use crate::initrd::InitrdNodeKind;
use crate::procfs;
use crate::procfs::ProcfsNodeKind;

const ROOT_PATH: &str = "/";
const DEV_ROOT_PATH: &str = "/dev";
const PROC_ROOT_PATH: &str = "/proc";
const INITRD_ROOT_PATH: &str = "/initrd";
const INIT_PATH: &str = "/initrd/init";

struct GlobalVfs(UnsafeCell<Option<VfsState>>);

unsafe impl Sync for GlobalVfs {}

impl GlobalVfs {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<VfsState> {
        self.0.get()
    }
}

static VFS: GlobalVfs = GlobalVfs::new();

#[derive(Copy, Clone)]
struct VfsState {
    initialized: bool,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum VfsMountKind {
    Root,
    Devfs,
    Initrd,
    Procfs,
}

impl VfsMountKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Root => "rootfs",
            Self::Devfs => "devfs",
            Self::Initrd => "initrd",
            Self::Procfs => "procfs",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum VfsNodeKind {
    Directory,
    File,
    Device,
}

pub struct VfsNode {
    pub path: String,
    pub mount: VfsMountKind,
    pub kind: VfsNodeKind,
    pub size: usize,
    pub executable: bool,
}

#[derive(Copy, Clone)]
pub struct VfsSummary {
    pub mount_count: usize,
    pub root_entry_count: usize,
    pub directory_count: usize,
}

#[derive(Copy, Clone)]
pub enum VfsError {
    AlreadyInitialized,
}

impl VfsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "vfs is already initialized",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum ExecutableFormat {
    Elf,
    ShebangScript,
    Text,
    Unknown,
}

impl ExecutableFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Elf => "elf",
            Self::ShebangScript => "script-shebang",
            Self::Text => "text",
            Self::Unknown => "unknown",
        }
    }
}

pub struct ExecutableCandidate {
    pub path: String,
    pub mount: VfsMountKind,
    pub format: ExecutableFormat,
    pub size: usize,
    pub executable: bool,
}

pub struct VmMapPlanEntry {
    pub index: usize,
    pub file_offset: u64,
    pub virtual_start: u64,
    pub virtual_end: u64,
    pub map_start: u64,
    pub map_end: u64,
    pub page_offset: u64,
    pub file_bytes: u64,
    pub memory_bytes: u64,
    pub zero_fill_bytes: u64,
    pub alignment: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
}

pub struct VmMapImageEntry {
    pub index: usize,
    pub file_offset: u64,
    pub virtual_start: u64,
    pub virtual_end: u64,
    pub map_start: u64,
    pub map_end: u64,
    pub page_offset: u64,
    pub file_bytes: u64,
    pub memory_bytes: u64,
    pub zero_fill_bytes: u64,
    pub alignment: u64,
    pub readable: bool,
    pub writable: bool,
    pub executable: bool,
    pub bytes: Vec<u8>,
}

#[derive(Copy, Clone)]
pub enum ExecutableDiscoveryError {
    VfsUnavailable,
    PathNotFound,
    NotAFile,
    BackendUnavailable,
    ParseFailed(exec::ParseError),
}

impl ExecutableDiscoveryError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VfsUnavailable => "vfs is not initialized",
            Self::PathNotFound => "executable path was not found",
            Self::NotAFile => "executable path resolved to a non-file node",
            Self::BackendUnavailable => "backend cannot provide executable bytes",
            Self::ParseFailed(error) => error.as_str(),
        }
    }
}

pub struct ExecutableLoadPrep {
    pub path: String,
    pub mount: VfsMountKind,
    pub format: ExecutableFormat,
    pub size: usize,
    pub executable: bool,
    pub image_type: Option<u16>,
    pub machine: Option<u16>,
    pub entry_point: Option<u64>,
    pub program_header_count: usize,
    pub load_segment_count: usize,
    pub load_base: Option<u64>,
    pub load_offset: Option<u64>,
    pub load_file_bytes: u64,
    pub load_memory_bytes: u64,
    pub writable_load_segments: usize,
    pub executable_load_segments: usize,
    pub max_alignment: u64,
    pub vm_map_entries: Vec<VmMapPlanEntry>,
    pub vm_map_total_bytes: u64,
    pub vm_map_zero_fill_bytes: u64,
    pub vm_map_start: Option<u64>,
    pub vm_map_end: Option<u64>,
    pub interpreter: Option<String>,
    pub interpreter_source: Option<String>,
    pub interpreter_argument: Option<String>,
    pub interpreter_resolved: bool,
}

pub struct ExecutableLoadImage {
    pub path: String,
    pub mount: VfsMountKind,
    pub format: ExecutableFormat,
    pub size: usize,
    pub executable: bool,
    pub image_type: Option<u16>,
    pub machine: Option<u16>,
    pub entry_point: Option<u64>,
    pub interpreter: Option<String>,
    pub interpreter_source: Option<String>,
    pub interpreter_argument: Option<String>,
    pub interpreter_resolved: bool,
    pub vm_map_images: Vec<VmMapImageEntry>,
    pub vm_map_total_bytes: u64,
    pub vm_map_zero_fill_bytes: u64,
}

#[derive(Copy, Clone)]
pub enum ExecutableLoadPrepError {
    Discovery(ExecutableDiscoveryError),
    Parse(exec::ParseError),
}

impl ExecutableLoadPrepError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovery(error) => error.as_str(),
            Self::Parse(error) => error.as_str(),
        }
    }
}

pub fn initialize() -> Result<VfsSummary, VfsError> {
    let slot = unsafe { &mut *VFS.get() };
    if slot.is_some() {
        return Err(VfsError::AlreadyInitialized);
    }

    *slot = Some(VfsState { initialized: true });
    Ok(summary())
}

pub fn summary() -> VfsSummary {
    let initialized = unsafe { (&*VFS.get()).as_ref().is_some_and(|state| state.initialized) };
    if !initialized {
        return VfsSummary {
            mount_count: 0,
            root_entry_count: 0,
            directory_count: 0,
        };
    }

    let initrd_online = initrd::is_initialized();
    let mount_count = 2 + usize::from(initrd_online);
    let directory_count = 3
        + if initrd_online {
            initrd::summary().directory_count
        } else {
            0
        };

    VfsSummary {
        mount_count,
        root_entry_count: mount_count,
        directory_count,
    }
}

pub fn lookup(path: &str) -> Option<VfsNode> {
    let _state = unsafe { (&*VFS.get()).as_ref()? };
    let normalized = normalize_path(path)?;
    resolve_node(&normalized)
}

pub fn read(path: &str) -> Option<String> {
    let node = lookup(path)?;
    match node.mount {
        VfsMountKind::Root => Some(render_root()),
        VfsMountKind::Devfs => devfs::read(&node.path),
        VfsMountKind::Initrd => initrd::read(&node.path),
        VfsMountKind::Procfs => procfs::read(&node.path),
    }
}

pub fn preview(path: &str, max_len: usize) -> Option<String> {
    let normalized = normalize_path(path)?;
    let content = read(&normalized)?;
    if normalized == ROOT_PATH {
        let mut preview = String::new();
        for entry in content.lines() {
            if !preview.is_empty() {
                preview.push(' ');
            }
            preview.push_str(entry.trim());
        }
        if preview.len() <= max_len {
            return Some(preview);
        }

        let mut truncated = String::new();
        truncated.push_str(&preview[..max_len]);
        truncated.push_str("...");
        return Some(truncated);
    }

    let line = content.lines().next()?.trim();
    if line.len() <= max_len {
        return Some(String::from(line));
    }

    let mut preview = String::new();
    preview.push_str(&line[..max_len]);
    preview.push_str("...");
    Some(preview)
}

pub fn discover_init_executable() -> Result<ExecutableCandidate, ExecutableDiscoveryError> {
    discover_executable(INIT_PATH)
}

pub fn discover_executable(path: &str) -> Result<ExecutableCandidate, ExecutableDiscoveryError> {
    if !unsafe { (&*VFS.get()).as_ref().is_some_and(|state| state.initialized) } {
        return Err(ExecutableDiscoveryError::VfsUnavailable);
    }

    let node = lookup(path).ok_or(ExecutableDiscoveryError::PathNotFound)?;
    if node.kind != VfsNodeKind::File {
        return Err(ExecutableDiscoveryError::NotAFile);
    }

    let bytes =
        read_executable_bytes(node.mount, &node.path).ok_or(ExecutableDiscoveryError::BackendUnavailable)?;
    let kind = exec::detect_kind(bytes).map_err(ExecutableDiscoveryError::ParseFailed)?;

    Ok(ExecutableCandidate {
        path: node.path,
        mount: node.mount,
        format: executable_format_from_kind(kind),
        size: node.size,
        executable: node.executable,
    })
}

pub fn prepare_init_load() -> Result<ExecutableLoadPrep, ExecutableLoadPrepError> {
    prepare_executable_load(INIT_PATH)
}

pub fn materialize_init_image() -> Result<ExecutableLoadImage, ExecutableLoadPrepError> {
    materialize_executable_image(INIT_PATH)
}

pub fn prepare_executable_load(path: &str) -> Result<ExecutableLoadPrep, ExecutableLoadPrepError> {
    let candidate = discover_executable(path).map_err(ExecutableLoadPrepError::Discovery)?;
    let bytes = read_executable_bytes(candidate.mount, &candidate.path).ok_or(
        ExecutableLoadPrepError::Discovery(ExecutableDiscoveryError::BackendUnavailable),
    )?;
    let image = exec::inspect(bytes).map_err(ExecutableLoadPrepError::Parse)?;

    match image {
        exec::ExecutableImage::Elf64(elf) => {
            let load_plan = exec::build_load_plan(&elf).map_err(ExecutableLoadPrepError::Parse)?;
            let mut load_segment_count = 0usize;
            let mut load_base = None;
            let mut load_offset = None;
            let mut load_file_bytes = 0u64;
            let mut load_memory_bytes = 0u64;
            let mut writable_load_segments = 0usize;
            let mut executable_load_segments = 0usize;
            let mut max_alignment = 0u64;
            let mut vm_map_entries = Vec::with_capacity(load_plan.len());
            let mut vm_map_total_bytes = 0u64;
            let mut vm_map_zero_fill_bytes = 0u64;
            let mut vm_map_start = None;
            let mut vm_map_end = None;
            for header in load_plan {
                load_segment_count += 1;
                load_base = Some(
                    load_base.map_or(header.virtual_start, |base: u64| {
                        base.min(header.virtual_start)
                    }),
                );
                load_offset = Some(load_offset.map_or(header.file_offset, |offset: u64| {
                    offset.min(header.file_offset)
                }));
                load_file_bytes = load_file_bytes.saturating_add(header.file_bytes);
                load_memory_bytes = load_memory_bytes.saturating_add(header.memory_bytes);
                if header.permissions.write {
                    writable_load_segments += 1;
                }
                if header.permissions.execute {
                    executable_load_segments += 1;
                }
                max_alignment = max_alignment.max(header.alignment);
                vm_map_total_bytes =
                    vm_map_total_bytes.saturating_add(header.map_end.saturating_sub(header.map_start));
                vm_map_zero_fill_bytes =
                    vm_map_zero_fill_bytes.saturating_add(header.zero_fill_bytes);
                vm_map_start = Some(vm_map_start.map_or(header.map_start, |start: u64| {
                    start.min(header.map_start)
                }));
                vm_map_end = Some(vm_map_end.map_or(header.map_end, |end: u64| {
                    end.max(header.map_end)
                }));
                vm_map_entries.push(VmMapPlanEntry {
                    index: header.index,
                    file_offset: header.file_offset,
                    virtual_start: header.virtual_start,
                    virtual_end: header.virtual_end,
                    map_start: header.map_start,
                    map_end: header.map_end,
                    page_offset: header.page_offset,
                    file_bytes: header.file_bytes,
                    memory_bytes: header.memory_bytes,
                    zero_fill_bytes: header.zero_fill_bytes,
                    alignment: header.alignment,
                    readable: header.permissions.read,
                    writable: header.permissions.write,
                    executable: header.permissions.execute,
                });
            }
            let interpreter = elf.interpreter;
            let interpreter_source = interpreter
                .as_deref()
                .and_then(resolve_runtime_path);
            let interpreter_resolved = interpreter_source.is_some();
            Ok(ExecutableLoadPrep {
                path: candidate.path,
                mount: candidate.mount,
                format: candidate.format,
                size: candidate.size,
                executable: candidate.executable,
                image_type: Some(elf.image_type),
                machine: Some(elf.machine),
                entry_point: Some(elf.entry_point),
                program_header_count: elf.program_headers.len(),
                load_segment_count,
                load_base,
                load_offset,
                load_file_bytes,
                load_memory_bytes,
                writable_load_segments,
                executable_load_segments,
                max_alignment,
                vm_map_entries,
                vm_map_total_bytes,
                vm_map_zero_fill_bytes,
                vm_map_start,
                vm_map_end,
                interpreter,
                interpreter_source,
                interpreter_argument: None,
                interpreter_resolved,
            })
        }
        exec::ExecutableImage::Shebang(script) => {
            let interpreter_source = resolve_runtime_path(&script.interpreter);
            let interpreter_resolved = interpreter_source.is_some();
            Ok(ExecutableLoadPrep {
                path: candidate.path,
                mount: candidate.mount,
                format: candidate.format,
                size: candidate.size,
                executable: candidate.executable,
                image_type: None,
                machine: None,
                entry_point: None,
                program_header_count: 0,
                load_segment_count: 0,
                load_base: None,
                load_offset: None,
                load_file_bytes: 0,
                load_memory_bytes: 0,
                writable_load_segments: 0,
                executable_load_segments: 0,
                max_alignment: 0,
                vm_map_entries: Vec::new(),
                vm_map_total_bytes: 0,
                vm_map_zero_fill_bytes: 0,
                vm_map_start: None,
                vm_map_end: None,
                interpreter: Some(script.interpreter),
                interpreter_source,
                interpreter_argument: script.argument,
                interpreter_resolved,
            })
        }
        exec::ExecutableImage::Text | exec::ExecutableImage::Unknown => Ok(ExecutableLoadPrep {
            path: candidate.path,
            mount: candidate.mount,
            format: candidate.format,
            size: candidate.size,
            executable: candidate.executable,
            image_type: None,
            machine: None,
            entry_point: None,
            program_header_count: 0,
            load_segment_count: 0,
            load_base: None,
            load_offset: None,
            load_file_bytes: 0,
            load_memory_bytes: 0,
            writable_load_segments: 0,
            executable_load_segments: 0,
            max_alignment: 0,
            vm_map_entries: Vec::new(),
            vm_map_total_bytes: 0,
            vm_map_zero_fill_bytes: 0,
            vm_map_start: None,
            vm_map_end: None,
            interpreter: None,
            interpreter_source: None,
            interpreter_argument: None,
            interpreter_resolved: false,
        }),
    }
}

pub fn materialize_executable_image(path: &str) -> Result<ExecutableLoadImage, ExecutableLoadPrepError> {
    let candidate = discover_executable(path).map_err(ExecutableLoadPrepError::Discovery)?;
    let bytes = read_executable_bytes(candidate.mount, &candidate.path).ok_or(
        ExecutableLoadPrepError::Discovery(ExecutableDiscoveryError::BackendUnavailable),
    )?;
    let image = exec::inspect(bytes).map_err(ExecutableLoadPrepError::Parse)?;

    match image {
        exec::ExecutableImage::Elf64(elf) => {
            let load_plan = exec::build_load_plan(&elf).map_err(ExecutableLoadPrepError::Parse)?;
            let mapped_segments =
                exec::materialize_load_segments(bytes, &load_plan).map_err(ExecutableLoadPrepError::Parse)?;
            let mut vm_map_images = Vec::with_capacity(load_plan.len());
            let mut vm_map_total_bytes = 0u64;
            let mut vm_map_zero_fill_bytes = 0u64;

            for (header, mapped_bytes) in load_plan.into_iter().zip(mapped_segments.into_iter()) {
                let mapped_len = u64::try_from(mapped_bytes.len())
                    .map_err(|_| ExecutableLoadPrepError::Parse(exec::ParseError::SegmentAddressOverflow))?;
                vm_map_total_bytes = vm_map_total_bytes.saturating_add(mapped_len);
                vm_map_zero_fill_bytes =
                    vm_map_zero_fill_bytes.saturating_add(header.zero_fill_bytes);
                vm_map_images.push(VmMapImageEntry {
                    index: header.index,
                    file_offset: header.file_offset,
                    virtual_start: header.virtual_start,
                    virtual_end: header.virtual_end,
                    map_start: header.map_start,
                    map_end: header.map_end,
                    page_offset: header.page_offset,
                    file_bytes: header.file_bytes,
                    memory_bytes: header.memory_bytes,
                    zero_fill_bytes: header.zero_fill_bytes,
                    alignment: header.alignment,
                    readable: header.permissions.read,
                    writable: header.permissions.write,
                    executable: header.permissions.execute,
                    bytes: mapped_bytes,
                });
            }

            let interpreter = elf.interpreter;
            let interpreter_source = interpreter.as_deref().and_then(resolve_runtime_path);
            let interpreter_resolved = interpreter_source.is_some();
            Ok(ExecutableLoadImage {
                path: candidate.path,
                mount: candidate.mount,
                format: candidate.format,
                size: candidate.size,
                executable: candidate.executable,
                image_type: Some(elf.image_type),
                machine: Some(elf.machine),
                entry_point: Some(elf.entry_point),
                interpreter,
                interpreter_source,
                interpreter_argument: None,
                interpreter_resolved,
                vm_map_images,
                vm_map_total_bytes,
                vm_map_zero_fill_bytes,
            })
        }
        exec::ExecutableImage::Shebang(script) => {
            let interpreter_source = resolve_runtime_path(&script.interpreter);
            let interpreter_resolved = interpreter_source.is_some();
            Ok(ExecutableLoadImage {
                path: candidate.path,
                mount: candidate.mount,
                format: candidate.format,
                size: candidate.size,
                executable: candidate.executable,
                image_type: None,
                machine: None,
                entry_point: None,
                interpreter: Some(script.interpreter),
                interpreter_source,
                interpreter_argument: script.argument,
                interpreter_resolved,
                vm_map_images: Vec::new(),
                vm_map_total_bytes: 0,
                vm_map_zero_fill_bytes: 0,
            })
        }
        exec::ExecutableImage::Text | exec::ExecutableImage::Unknown => Ok(ExecutableLoadImage {
            path: candidate.path,
            mount: candidate.mount,
            format: candidate.format,
            size: candidate.size,
            executable: candidate.executable,
            image_type: None,
            machine: None,
            entry_point: None,
            interpreter: None,
            interpreter_source: None,
            interpreter_argument: None,
            interpreter_resolved: false,
            vm_map_images: Vec::new(),
            vm_map_total_bytes: 0,
            vm_map_zero_fill_bytes: 0,
        }),
    }
}

fn resolve_node(path: &str) -> Option<VfsNode> {
    match path {
        ROOT_PATH => Some(VfsNode {
            path: String::from(ROOT_PATH),
            mount: VfsMountKind::Root,
            kind: VfsNodeKind::Directory,
            size: render_root().len(),
            executable: false,
        }),
        _ if path == DEV_ROOT_PATH || path.starts_with("/dev/") => resolve_devfs_node(path),
        _ if path == INITRD_ROOT_PATH || path.starts_with("/initrd/") => resolve_initrd_node(path),
        _ if path == PROC_ROOT_PATH || path.starts_with("/proc/") => resolve_procfs_node(path),
        _ => None,
    }
}

fn resolve_devfs_node(path: &str) -> Option<VfsNode> {
    let kind = match devfs::node_kind(path)? {
        DevfsNodeKind::Directory => VfsNodeKind::Directory,
        DevfsNodeKind::Device => VfsNodeKind::Device,
    };

    let size = devfs::read(path).map_or(0, |content| content.len());

    Some(VfsNode {
        path: String::from(path),
        mount: VfsMountKind::Devfs,
        kind,
        size,
        executable: false,
    })
}

fn resolve_procfs_node(path: &str) -> Option<VfsNode> {
    let kind = match procfs::node_kind(path)? {
        ProcfsNodeKind::Directory => VfsNodeKind::Directory,
        ProcfsNodeKind::File => VfsNodeKind::File,
    };

    let size = procfs::read(path).map_or(0, |content| content.len());

    Some(VfsNode {
        path: String::from(path),
        mount: VfsMountKind::Procfs,
        kind,
        size,
        executable: false,
    })
}

fn resolve_initrd_node(path: &str) -> Option<VfsNode> {
    let info = initrd::node_info(path)?;
    let kind = match info.kind {
        InitrdNodeKind::Directory => VfsNodeKind::Directory,
        InitrdNodeKind::File => VfsNodeKind::File,
    };

    Some(VfsNode {
        path: String::from(path),
        mount: VfsMountKind::Initrd,
        kind,
        size: info.size,
        executable: info.executable,
    })
}

fn resolve_runtime_path(path: &str) -> Option<String> {
    let normalized = normalize_path(path)?;
    if let Some(node) = lookup(&normalized) {
        return Some(node.path);
    }
    if normalized == ROOT_PATH || normalized.starts_with(INITRD_ROOT_PATH) {
        return None;
    }

    let mut initrd_path = String::from(INITRD_ROOT_PATH);
    initrd_path.push_str(&normalized);
    lookup(&initrd_path).map(|node| node.path)
}

fn render_root() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "dev");
    if initrd::is_initialized() {
        let _ = writeln!(text, "initrd");
    }
    let _ = writeln!(text, "proc");
    text
}

fn executable_format_from_kind(kind: exec::ImageKind) -> ExecutableFormat {
    match kind {
        exec::ImageKind::Elf64 => ExecutableFormat::Elf,
        exec::ImageKind::ShebangScript => ExecutableFormat::ShebangScript,
        exec::ImageKind::Text => ExecutableFormat::Text,
        exec::ImageKind::Unknown => ExecutableFormat::Unknown,
    }
}

fn read_executable_bytes(mount: VfsMountKind, path: &str) -> Option<&'static [u8]> {
    match mount {
        VfsMountKind::Initrd => initrd::read_bytes(path),
        _ => None,
    }
}

pub fn format_u16_hex(value: Option<u16>) -> String {
    match value {
        Some(value) => {
            let mut text = String::from("0x");
            text.push_str(&hex_u16(value));
            text
        }
        None => String::from("<none>"),
    }
}

fn hex_u16(value: u16) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    for shift in [12, 8, 4, 0] {
        let nibble = ((value >> shift) & 0x0f) as usize;
        text.push(HEX[nibble] as char);
    }
    text
}

pub fn format_u64_hex(value: Option<u64>) -> String {
    match value {
        Some(value) => {
            let mut text = String::from("0x");
            text.push_str(&hex_u64(value));
            text
        }
        None => String::from("<none>"),
    }
}

pub const fn format_rwx(readable: bool, writable: bool, executable: bool) -> &'static str {
    match (readable, writable, executable) {
        (true, true, true) => "rwx",
        (true, true, false) => "rw-",
        (true, false, true) => "r-x",
        (true, false, false) => "r--",
        (false, true, true) => "-wx",
        (false, true, false) => "-w-",
        (false, false, true) => "--x",
        (false, false, false) => "---",
    }
}

fn hex_u64(value: u64) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    for shift in [60, 56, 52, 48, 44, 40, 36, 32, 28, 24, 20, 16, 12, 8, 4, 0] {
        let nibble = ((value >> shift) & 0x0f) as usize;
        text.push(HEX[nibble] as char);
    }
    text
}

fn normalize_path(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        return None;
    }

    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            segments.pop()?;
            continue;
        }

        segments.push(segment);
    }

    if segments.is_empty() {
        return Some(String::from(ROOT_PATH));
    }

    let mut normalized = String::from(ROOT_PATH);
    for (index, segment) in segments.iter().enumerate() {
        if index > 0 {
            normalized.push('/');
        }
        normalized.push_str(segment);
    }

    Some(normalized)
}
