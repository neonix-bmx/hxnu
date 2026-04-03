use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;
use core::str;

use crate::limine;

const INITRD_ROOT: &str = "/initrd";
const NEWC_MAGIC: &[u8; 6] = b"070701";
const CRC_MAGIC: &[u8; 6] = b"070702";
const CPIO_HEADER_LEN: usize = 110;
const FILE_TYPE_MASK: u32 = 0o170000;
const FILE_TYPE_DIRECTORY: u32 = 0o040000;
const MODE_EXECUTABLE_MASK: u32 = 0o111;
const DEFAULT_DIRECTORY_MODE: u32 = FILE_TYPE_DIRECTORY | 0o755;

struct GlobalInitrd(UnsafeCell<Option<InitrdState>>);

unsafe impl Sync for GlobalInitrd {}

impl GlobalInitrd {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<InitrdState> {
        self.0.get()
    }
}

static INITRD: GlobalInitrd = GlobalInitrd::new();

struct InitrdState {
    module_path: Option<&'static str>,
    module_label: Option<&'static str>,
    module_count: usize,
    archive: &'static [u8],
    archive_bytes: usize,
    entries: Vec<InitrdEntry>,
}

struct InitrdEntry {
    path: String,
    kind: InitrdEntryKind,
    mode: u32,
    data: &'static [u8],
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum InitrdEntryKind {
    Directory,
    File,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum InitrdNodeKind {
    Directory,
    File,
}

#[derive(Copy, Clone)]
pub struct InitrdNodeInfo {
    pub kind: InitrdNodeKind,
    pub size: usize,
    pub executable: bool,
}

#[derive(Copy, Clone)]
pub struct InitrdSummary {
    pub module_count: usize,
    pub archive_bytes: usize,
    pub directory_count: usize,
    pub file_count: usize,
    pub entry_count: usize,
}

#[derive(Copy, Clone)]
pub enum InitrdError {
    AlreadyInitialized,
    ModuleMissing,
    InvalidArchive,
    InvalidEntryName,
    UnsupportedPath,
}

impl InitrdError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "initrd is already initialized",
            Self::ModuleMissing => "initrd module not found",
            Self::InvalidArchive => "initrd archive is malformed or unsupported",
            Self::InvalidEntryName => "initrd entry name is invalid",
            Self::UnsupportedPath => "initrd only supports /initrd paths",
        }
    }
}

pub fn initialize() -> Result<InitrdSummary, InitrdError> {
    let slot = unsafe { &mut *INITRD.get() };
    if slot.is_some() {
        return Err(InitrdError::AlreadyInitialized);
    }

    let modules = limine::modules().ok_or(InitrdError::ModuleMissing)?;
    if modules.is_empty() {
        return Err(InitrdError::ModuleMissing);
    }

    let module = select_initrd_module(&modules).ok_or(InitrdError::ModuleMissing)?;
    let archive = module.bytes();
    let entries = parse_newc_archive(archive)?;
    *slot = Some(InitrdState {
        module_path: module.path(),
        module_label: module.string(),
        module_count: modules.len(),
        archive,
        archive_bytes: module.size(),
        entries,
    });

    Ok(summary())
}

pub fn is_initialized() -> bool {
    unsafe { (&*INITRD.get()).is_some() }
}

pub fn summary() -> InitrdSummary {
    let Some(state) = (unsafe { (&*INITRD.get()).as_ref() }) else {
        return InitrdSummary {
            module_count: 0,
            archive_bytes: 0,
            directory_count: 0,
            file_count: 0,
            entry_count: 0,
        };
    };

    let mut directory_count = 0;
    let mut file_count = 0;
    for entry in &state.entries {
        match entry.kind {
            InitrdEntryKind::Directory => directory_count += 1,
            InitrdEntryKind::File => file_count += 1,
        }
    }

    InitrdSummary {
        module_count: state.module_count,
        archive_bytes: state.archive_bytes,
        directory_count,
        file_count,
        entry_count: state.entries.len(),
    }
}

pub fn module_path() -> Option<&'static str> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    state.module_path
}

pub fn module_label() -> Option<&'static str> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    state.module_label
}

pub fn archive_bytes() -> Option<&'static [u8]> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    Some(state.archive)
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    let normalized = normalize_vfs_path(path).ok()?;

    let entry = find_entry(state, &normalized)?;
    match entry.kind {
        InitrdEntryKind::Directory => Some(render_directory(state, &normalized)),
        InitrdEntryKind::File => Some(String::from_utf8_lossy(entry.data).into_owned()),
    }
}

pub fn node_info(path: &str) -> Option<InitrdNodeInfo> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    let normalized = normalize_vfs_path(path).ok()?;
    let entry = find_entry(state, &normalized)?;

    let kind = match entry.kind {
        InitrdEntryKind::Directory => InitrdNodeKind::Directory,
        InitrdEntryKind::File => InitrdNodeKind::File,
    };

    Some(InitrdNodeInfo {
        kind,
        size: entry.data.len(),
        executable: entry.mode & MODE_EXECUTABLE_MASK != 0,
    })
}

pub fn read_bytes(path: &str) -> Option<&'static [u8]> {
    let state = unsafe { (&*INITRD.get()).as_ref()? };
    let normalized = normalize_vfs_path(path).ok()?;
    let entry = find_entry(state, &normalized)?;
    if entry.kind != InitrdEntryKind::File {
        return None;
    }

    Some(entry.data)
}

fn find_entry<'a>(state: &'a InitrdState, path: &str) -> Option<&'a InitrdEntry> {
    state.entries.iter().find(|entry| entry.path == path)
}

fn render_directory(state: &InitrdState, path: &str) -> String {
    let mut text = String::new();
    let prefix = if path == INITRD_ROOT {
        format!("{INITRD_ROOT}/")
    } else {
        format!("{path}/")
    };

    for entry in &state.entries {
        if entry.path == path || !entry.path.starts_with(&prefix) {
            continue;
        }

        let remainder = &entry.path[prefix.len()..];
        if remainder.is_empty() || remainder.contains('/') {
            continue;
        }

        let _ = writeln!(text, "{}", remainder);
    }

    text
}

fn parse_newc_archive(archive: &'static [u8]) -> Result<Vec<InitrdEntry>, InitrdError> {
    let mut cursor = 0usize;
    let mut entries = Vec::new();
    push_directory(&mut entries, INITRD_ROOT, DEFAULT_DIRECTORY_MODE);
    let mut found_trailer = false;

    while cursor + CPIO_HEADER_LEN <= archive.len() {
        let header = &archive[cursor..cursor + CPIO_HEADER_LEN];
        cursor += CPIO_HEADER_LEN;

        let magic = &header[0..6];
        if magic != NEWC_MAGIC && magic != CRC_MAGIC {
            return Err(InitrdError::InvalidArchive);
        }

        let mode = parse_hex_u32(&header[14..22])?;
        let file_size = parse_hex_u32(&header[54..62])? as usize;
        let name_size = parse_hex_u32(&header[94..102])? as usize;
        if name_size == 0 || cursor + name_size > archive.len() {
            return Err(InitrdError::InvalidArchive);
        }

        let name_bytes = &archive[cursor..cursor + name_size];
        cursor += name_size;
        cursor = align4(cursor);
        if cursor > archive.len() {
            return Err(InitrdError::InvalidArchive);
        }

        let raw_name = name_bytes.strip_suffix(&[0]).ok_or(InitrdError::InvalidArchive)?;
        let raw_name = str::from_utf8(raw_name).map_err(|_| InitrdError::InvalidEntryName)?;
        if raw_name == "TRAILER!!!" {
            found_trailer = true;
            break;
        }

        let normalized = normalize_archive_path(raw_name)?;
        ensure_parent_directories(&mut entries, &normalized);

        if cursor + file_size > archive.len() {
            return Err(InitrdError::InvalidArchive);
        }
        let file_data = &archive[cursor..cursor + file_size];
        cursor += file_size;
        cursor = align4(cursor);
        if cursor > archive.len() {
            return Err(InitrdError::InvalidArchive);
        }

        if mode & FILE_TYPE_MASK == FILE_TYPE_DIRECTORY {
            push_directory(&mut entries, &normalized, mode);
        } else {
            entries.push(InitrdEntry {
                path: normalized,
                kind: InitrdEntryKind::File,
                mode,
                data: file_data,
            });
        }
    }

    if !found_trailer {
        return Err(InitrdError::InvalidArchive);
    }

    Ok(entries)
}

fn select_initrd_module(modules: &limine::ModuleList) -> Option<limine::Module> {
    let mut first = None;
    for module in modules.iter() {
        if first.is_none() {
            first = Some(module);
        }

        if matches!(module.string(), Some("initrd")) {
            return Some(module);
        }
        if matches!(module.path(), Some(path) if path.ends_with("initrd.cpio")) {
            return Some(module);
        }
    }

    first
}

fn ensure_parent_directories(entries: &mut Vec<InitrdEntry>, path: &str) {
    if path.len() <= INITRD_ROOT.len() {
        return;
    }

    let mut end = INITRD_ROOT.len();
    while let Some(offset) = path[end + 1..].find('/') {
        end += offset + 1;
        push_directory(entries, &path[..end], DEFAULT_DIRECTORY_MODE);
    }
}

fn push_directory(entries: &mut Vec<InitrdEntry>, path: &str, mode: u32) {
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.kind == InitrdEntryKind::Directory && entry.path == path)
    {
        entry.mode = mode;
        return;
    }

    entries.push(InitrdEntry {
        path: String::from(path),
        kind: InitrdEntryKind::Directory,
        mode,
        data: &[],
    });
}

fn normalize_archive_path(path: &str) -> Result<String, InitrdError> {
    let trimmed = path.trim_matches('/');
    let trimmed = trimmed.strip_prefix("./").unwrap_or(trimmed);
    if trimmed.is_empty() || trimmed == "." {
        return Ok(String::from(INITRD_ROOT));
    }
    if trimmed.contains("..") {
        return Err(InitrdError::InvalidEntryName);
    }

    let mut normalized = String::from(INITRD_ROOT);
    normalized.push('/');
    normalized.push_str(trimmed);
    Ok(normalized)
}

fn normalize_vfs_path(path: &str) -> Result<String, InitrdError> {
    if path == INITRD_ROOT || path == "/initrd/" {
        return Ok(String::from(INITRD_ROOT));
    }
    if !path.starts_with("/initrd/") {
        return Err(InitrdError::UnsupportedPath);
    }

    let trimmed = path.trim_end_matches('/');
    Ok(String::from(trimmed))
}

fn parse_hex_u32(bytes: &[u8]) -> Result<u32, InitrdError> {
    let mut value = 0u32;
    for byte in bytes {
        value = value
            .checked_mul(16)
            .ok_or(InitrdError::InvalidArchive)?
            + match byte {
                b'0'..=b'9' => (byte - b'0') as u32,
                b'a'..=b'f' => (byte - b'a' + 10) as u32,
                b'A'..=b'F' => (byte - b'A' + 10) as u32,
                _ => return Err(InitrdError::InvalidArchive),
            };
    }
    Ok(value)
}

const fn align4(value: usize) -> usize {
    (value + 3) & !3
}
