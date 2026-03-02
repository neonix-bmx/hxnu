use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::tty;

const DEVFS_DIRECTORIES: [&str; 2] = ["/", "/dev"];
const DEVFS_NODES: [&str; 5] = [
    "/dev/console",
    "/dev/tty0",
    "/dev/null",
    "/dev/zero",
    "/dev/kmsg",
];

struct GlobalDevfs(UnsafeCell<Option<DevfsState>>);

unsafe impl Sync for GlobalDevfs {}

impl GlobalDevfs {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<DevfsState> {
        self.0.get()
    }
}

static DEVFS: GlobalDevfs = GlobalDevfs::new();

#[derive(Copy, Clone)]
struct DevfsState {
    boot_console_id: u32,
    boot_output_count: u8,
}

#[derive(Copy, Clone)]
pub struct DevfsSummary {
    pub directory_count: usize,
    pub node_count: usize,
    pub entry_count: usize,
}

#[derive(Copy, Clone)]
pub enum DevfsError {
    AlreadyInitialized,
}

impl DevfsError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "devfs is already initialized",
        }
    }
}

pub fn initialize() -> Result<DevfsSummary, DevfsError> {
    let slot = unsafe { &mut *DEVFS.get() };
    if slot.is_some() {
        return Err(DevfsError::AlreadyInitialized);
    }

    let tty = tty::stats();
    *slot = Some(DevfsState {
        boot_console_id: tty.console_id,
        boot_output_count: tty.output_count,
    });

    Ok(summary())
}

pub fn summary() -> DevfsSummary {
    DevfsSummary {
        directory_count: DEVFS_DIRECTORIES.len(),
        node_count: DEVFS_NODES.len(),
        entry_count: DEVFS_DIRECTORIES.len() + DEVFS_NODES.len(),
    }
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*DEVFS.get()).as_ref()? };
    match path {
        "/dev" => Some(render_root()),
        "/dev/console" => Some(render_console(state, "/dev/console")),
        "/dev/tty0" => Some(render_console(state, "/dev/tty0")),
        "/dev/null" => Some(render_null()),
        "/dev/zero" => Some(render_zero()),
        "/dev/kmsg" => Some(render_kmsg()),
        _ => None,
    }
}

pub fn preview(path: &str, max_len: usize) -> Option<String> {
    let content = read(path)?;
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
    for node in DEVFS_NODES {
        let _ = writeln!(text, "{}", node.trim_start_matches("/dev/"));
    }
    text
}

fn render_console(state: &DevfsState, path: &str) -> String {
    let mut text = String::new();
    let tty_stats = tty::stats();
    let _ = writeln!(text, "path {}", path);
    let _ = writeln!(text, "kind tty-console");
    let _ = writeln!(text, "console_id {}", tty_stats.console_id);
    let _ = writeln!(text, "outputs {}", tty_stats.output_count);
    let _ = writeln!(text, "bytes {}", tty_stats.bytes_written);
    let _ = writeln!(text, "lines {}", tty_stats.lines_written);
    let _ = writeln!(text, "boot_console_id {}", state.boot_console_id);
    let _ = writeln!(text, "boot_outputs {}", state.boot_output_count);
    text
}

fn render_null() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/null");
    let _ = writeln!(text, "kind sink");
    let _ = writeln!(text, "reads eof");
    let _ = writeln!(text, "writes discard");
    text
}

fn render_zero() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/zero");
    let _ = writeln!(text, "kind source");
    let _ = writeln!(text, "reads zero-fill");
    let _ = writeln!(text, "writes discard");
    text
}

fn render_kmsg() -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path /dev/kmsg");
    let _ = writeln!(text, "kind kernel-log");
    let _ = writeln!(text, "writes append");
    let _ = writeln!(text, "reads snapshot-unavailable");
    text
}
