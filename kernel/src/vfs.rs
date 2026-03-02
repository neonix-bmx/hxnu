use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::devfs;
use crate::procfs;

const ROOT_DIRECTORIES: [&str; 3] = ["/", "/dev", "/proc"];

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
    mount_count: usize,
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

    *slot = Some(VfsState { mount_count: 2 });
    Ok(summary())
}

pub fn summary() -> VfsSummary {
    let mount_count = unsafe { (&*VFS.get()).as_ref().map_or(0, |state| state.mount_count) };
    VfsSummary {
        mount_count,
        root_entry_count: mount_count,
        directory_count: ROOT_DIRECTORIES.len(),
    }
}

pub fn read(path: &str) -> Option<String> {
    let _state = unsafe { (&*VFS.get()).as_ref()? };
    match path {
        "/" => Some(render_root()),
        "/dev" | "/dev/" => devfs::read("/dev"),
        "/proc" | "/proc/" => procfs::read("/proc"),
        _ if path.starts_with("/dev/") => devfs::read(path),
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
    let _ = writeln!(text, "proc");
    text
}
