use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::devfs;
use crate::devfs::DevfsNodeKind;
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

#[derive(Copy, Clone)]
pub enum ExecutableDiscoveryError {
    VfsUnavailable,
    PathNotFound,
    NotAFile,
    BackendUnavailable,
}

impl ExecutableDiscoveryError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VfsUnavailable => "vfs is not initialized",
            Self::PathNotFound => "executable path was not found",
            Self::NotAFile => "executable path resolved to a non-file node",
            Self::BackendUnavailable => "backend cannot provide executable bytes",
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

    let bytes = match node.mount {
        VfsMountKind::Initrd => initrd::read_bytes(&node.path),
        _ => None,
    }
    .ok_or(ExecutableDiscoveryError::BackendUnavailable)?;

    Ok(ExecutableCandidate {
        path: node.path,
        mount: node.mount,
        format: detect_executable_format(bytes),
        size: node.size,
        executable: node.executable,
    })
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

fn render_root() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "dev");
    if initrd::is_initialized() {
        let _ = writeln!(text, "initrd");
    }
    let _ = writeln!(text, "proc");
    text
}

fn detect_executable_format(bytes: &[u8]) -> ExecutableFormat {
    if bytes.starts_with(b"\x7FELF") {
        return ExecutableFormat::Elf;
    }
    if bytes.starts_with(b"#!") {
        return ExecutableFormat::ShebangScript;
    }
    if looks_like_text(bytes) {
        return ExecutableFormat::Text;
    }

    ExecutableFormat::Unknown
}

fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    bytes.iter().all(|byte| {
        matches!(
            byte,
            b'\n' | b'\r' | b'\t' | b' '..=b'~'
        )
    })
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
