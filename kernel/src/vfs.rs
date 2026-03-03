use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::devfs;
use crate::initrd;
use crate::procfs;

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
    let mount_count = if initialized { 2 + usize::from(initrd::is_initialized()) } else { 0 };
    VfsSummary {
        mount_count,
        root_entry_count: mount_count,
        directory_count: 3 + usize::from(initrd::is_initialized()),
    }
}

pub fn read(path: &str) -> Option<String> {
    let _state = unsafe { (&*VFS.get()).as_ref()? };
    match path {
        "/" => Some(render_root()),
        "/dev" | "/dev/" => devfs::read("/dev"),
        "/initrd" | "/initrd/" => initrd::read("/initrd"),
        "/proc" | "/proc/" => procfs::read("/proc"),
        _ if path.starts_with("/dev/") => devfs::read(path),
        _ if path.starts_with("/initrd/") => initrd::read(path),
        _ if path.starts_with("/proc/") => procfs::read(path),
        _ => None,
    }
}

pub fn preview(path: &str, max_len: usize) -> Option<String> {
    let content = read(path)?;
    if path == "/" {
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

fn render_root() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "dev");
    if initrd::is_initialized() {
        let _ = writeln!(text, "initrd");
    }
    let _ = writeln!(text, "proc");
    text
}
