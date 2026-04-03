use alloc::string::String;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::block;
use crate::tty;

const DEVFS_DIRECTORIES: [&str; 2] = ["/", "/dev"];
const STATIC_DEVFS_NODES: [&str; 8] = [
    "/dev/console",
    "/dev/tty0",
    "/dev/tty1",
    "/dev/tty2",
    "/dev/tty3",
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

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum DevfsNodeKind {
    Directory,
    Device,
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
    let dynamic_nodes = dynamic_block_node_count();
    let node_count = STATIC_DEVFS_NODES.len().saturating_add(dynamic_nodes);
    DevfsSummary {
        directory_count: DEVFS_DIRECTORIES.len(),
        node_count,
        entry_count: DEVFS_DIRECTORIES.len() + node_count,
    }
}

pub fn node_kind(path: &str) -> Option<DevfsNodeKind> {
    match path {
        "/dev" | "/dev/" => Some(DevfsNodeKind::Directory),
        _ if STATIC_DEVFS_NODES.iter().any(|node| *node == path) => Some(DevfsNodeKind::Device),
        _ if resolve_block_device_path(path).is_some() => Some(DevfsNodeKind::Device),
        _ if resolve_block_partition_path(path).is_some() => Some(DevfsNodeKind::Device),
        _ => None,
    }
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*DEVFS.get()).as_ref()? };
    match path {
        "/dev" | "/dev/" => Some(render_root()),
        "/dev/console" => Some(render_console(state, "/dev/console")),
        "/dev/tty0" => Some(render_console(state, "/dev/tty0")),
        "/dev/tty1" => Some(render_console(state, "/dev/tty1")),
        "/dev/tty2" => Some(render_console(state, "/dev/tty2")),
        "/dev/tty3" => Some(render_console(state, "/dev/tty3")),
        "/dev/null" => Some(render_null()),
        "/dev/zero" => Some(render_zero()),
        "/dev/kmsg" => Some(render_kmsg()),
        _ => {
            if let Some(device) = resolve_block_device_path(path) {
                return Some(render_block_device(path, device));
            }
            if let Some(partition) = resolve_block_partition_path(path) {
                return Some(render_block_partition(path, partition));
            }
            None
        }
    }
}

fn render_root() -> String {
    let mut text = String::new();
    for node in STATIC_DEVFS_NODES {
        let _ = writeln!(text, "{}", node.trim_start_matches("/dev/"));
    }

    if block::is_initialized() {
        append_block_nodes(&mut text);
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

fn resolve_block_device_path(path: &str) -> Option<block::BlockDeviceInfo> {
    if !block::is_initialized() {
        return None;
    }
    let parsed = parse_block_path(path)?;
    if parsed.partition_index.is_some() {
        return None;
    }
    block::device(parsed.device_index)
}

fn resolve_block_partition_path(path: &str) -> Option<block::PartitionInfo> {
    if !block::is_initialized() {
        return None;
    }
    let parsed = parse_block_path(path)?;
    let partition_index = parsed.partition_index?;
    let device = block::device(parsed.device_index)?;
    find_device_partition(device.id, partition_index)
}

fn find_device_partition(device_id: u16, device_partition_index: usize) -> Option<block::PartitionInfo> {
    if device_partition_index == 0 {
        return None;
    }

    let mut matched = 0usize;
    let mut partition_index = 0usize;
    while partition_index < block::partition_count() {
        if let Some(partition) = block::partition(partition_index) {
            if partition.device_id == device_id {
                matched += 1;
                if matched == device_partition_index {
                    return Some(partition);
                }
            }
        }
        partition_index += 1;
    }
    None
}

fn parse_block_path(path: &str) -> Option<ParsedBlockPath> {
    parse_sd_path(path)
        .or_else(|| parse_nvme_path(path))
        .or_else(|| parse_nvm_path(path))
}

#[derive(Copy, Clone)]
struct ParsedBlockPath {
    device_index: usize,
    partition_index: Option<usize>,
}

fn parse_sd_path(path: &str) -> Option<ParsedBlockPath> {
    let suffix = path.strip_prefix("/dev/sd")?;
    if suffix.is_empty() {
        return None;
    }

    let mut letter_end = 0usize;
    for byte in suffix.bytes() {
        if byte.is_ascii_lowercase() {
            letter_end += 1;
            continue;
        }
        break;
    }
    if letter_end == 0 {
        return None;
    }

    let device_index = decode_sd_letters(&suffix[..letter_end])?;
    let remainder = &suffix[letter_end..];
    if remainder.is_empty() {
        return Some(ParsedBlockPath {
            device_index,
            partition_index: None,
        });
    }

    let partition_index = parse_positive_decimal(remainder)?;
    Some(ParsedBlockPath {
        device_index,
        partition_index: Some(partition_index),
    })
}

fn parse_nvme_path(path: &str) -> Option<ParsedBlockPath> {
    let suffix = path.strip_prefix("/dev/nvme")?;
    let (device_index, after_device) = parse_decimal_prefix(suffix)?;
    let after_n = after_device.strip_prefix('n')?;
    let (namespace, after_namespace) = parse_decimal_prefix(after_n)?;
    if namespace != 1 {
        return None;
    }

    if after_namespace.is_empty() {
        return Some(ParsedBlockPath {
            device_index,
            partition_index: None,
        });
    }

    let partition_suffix = after_namespace.strip_prefix('p')?;
    let partition_index = parse_positive_decimal(partition_suffix)?;
    Some(ParsedBlockPath {
        device_index,
        partition_index: Some(partition_index),
    })
}

fn parse_nvm_path(path: &str) -> Option<ParsedBlockPath> {
    let suffix = path.strip_prefix("/dev/nvm")?;
    let (device_index, after_device) = parse_decimal_prefix(suffix)?;
    let after_n = after_device.strip_prefix('n')?;
    if after_n.is_empty() {
        return Some(ParsedBlockPath {
            device_index,
            partition_index: None,
        });
    }

    let partition_suffix = after_n.strip_prefix('p')?;
    let partition_index = parse_positive_decimal(partition_suffix)?;
    Some(ParsedBlockPath {
        device_index,
        partition_index: Some(partition_index),
    })
}

fn decode_sd_letters(letters: &str) -> Option<usize> {
    let mut value = 0usize;
    for byte in letters.bytes() {
        if !byte.is_ascii_lowercase() {
            return None;
        }
        let digit = usize::from(byte - b'a' + 1);
        value = value.checked_mul(26)?;
        value = value.checked_add(digit)?;
    }
    value.checked_sub(1)
}

fn parse_positive_decimal(input: &str) -> Option<usize> {
    if input.is_empty() {
        return None;
    }
    let mut value = 0usize;
    for byte in input.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?;
        value = value.checked_add(usize::from(byte - b'0'))?;
    }
    if value == 0 {
        return None;
    }
    Some(value)
}

fn parse_decimal_prefix(input: &str) -> Option<(usize, &str)> {
    if input.is_empty() {
        return None;
    }
    let mut end = 0usize;
    for byte in input.bytes() {
        if byte.is_ascii_digit() {
            end += 1;
            continue;
        }
        break;
    }
    if end == 0 {
        return None;
    }
    let value = parse_unsigned_decimal(&input[..end])?;
    Some((value, &input[end..]))
}

fn parse_unsigned_decimal(input: &str) -> Option<usize> {
    if input.is_empty() {
        return None;
    }
    let mut value = 0usize;
    for byte in input.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?;
        value = value.checked_add(usize::from(byte - b'0'))?;
    }
    Some(value)
}

fn format_sd_disk_name(index: usize) -> String {
    let mut name = String::from("sd");
    name.push_str(&encode_sd_letters(index));
    name
}

fn format_nvme_disk_name(index: usize) -> String {
    let mut name = String::from("nvme");
    push_usize_decimal(&mut name, index);
    name.push_str("n1");
    name
}

fn format_nvm_disk_name(index: usize) -> String {
    let mut name = String::from("nvm");
    push_usize_decimal(&mut name, index);
    name.push('n');
    name
}

fn format_sd_partition_name(device_index: usize, partition_index: usize) -> String {
    let mut name = format_sd_disk_name(device_index);
    push_usize_decimal(&mut name, partition_index);
    name
}

fn format_nvme_partition_name(device_index: usize, partition_index: usize) -> String {
    let mut name = format_nvme_disk_name(device_index);
    name.push('p');
    push_usize_decimal(&mut name, partition_index);
    name
}

fn format_nvm_partition_name(device_index: usize, partition_index: usize) -> String {
    let mut name = format_nvm_disk_name(device_index);
    name.push('p');
    push_usize_decimal(&mut name, partition_index);
    name
}

fn push_usize_decimal(out: &mut String, mut value: usize) {
    if value == 0 {
        out.push('0');
        return;
    }

    let mut reversed = [0u8; 20];
    let mut len = 0usize;
    while value > 0 {
        reversed[len] = b'0' + (value % 10) as u8;
        len += 1;
        value /= 10;
    }

    while len > 0 {
        len -= 1;
        out.push(reversed[len] as char);
    }
}

fn encode_sd_letters(index: usize) -> String {
    let mut n = index.saturating_add(1);
    let mut reversed = [0u8; 16];
    let mut len = 0usize;
    while n > 0 {
        let rem = (n - 1) % 26;
        reversed[len] = b'a' + rem as u8;
        len += 1;
        n = (n - 1) / 26;
    }

    let mut text = String::new();
    while len > 0 {
        len -= 1;
        text.push(reversed[len] as char);
    }
    text
}

fn render_block_device(path: &str, device: block::BlockDeviceInfo) -> String {
    let mut text = String::new();
    let partition_count = count_device_partitions(device.id);
    let _ = writeln!(text, "path {}", path);
    let _ = writeln!(text, "kind block-device");
    let _ = writeln!(text, "id {}", device.id);
    let _ = writeln!(text, "driver {}", device.driver_name);
    let _ = writeln!(text, "driver-kind {}", device.kind.as_str());
    let _ = writeln!(text, "name {}", device.name);
    let _ = writeln!(text, "read-only {}", yes_no(device.read_only));
    let _ = writeln!(text, "sector-bytes {}", device.sector_size);
    let _ = writeln!(text, "sectors {}", device.sector_count);
    let _ = writeln!(text, "size-bytes {}", device.size_bytes);
    let _ = writeln!(text, "partitions {}", partition_count);
    text
}

fn render_block_partition(path: &str, partition: block::PartitionInfo) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "path {}", path);
    let _ = writeln!(text, "kind block-partition");
    let _ = writeln!(text, "id {}", partition.id);
    let _ = writeln!(text, "device-id {}", partition.device_id);
    let _ = writeln!(text, "table {}", partition.table_kind.as_str());
    let _ = writeln!(text, "lba-start {}", partition.start_lba);
    let _ = writeln!(text, "sectors {}", partition.sector_count);
    let _ = writeln!(text, "size-bytes {}", partition.sector_count.saturating_mul(512));
    match partition.table_kind {
        block::PartitionTableKind::Mbr => {
            let _ = writeln!(text, "mbr-index {}", partition.mbr_index);
            let _ = writeln!(text, "mbr-type {:#04x}", partition.partition_type);
            let _ = writeln!(text, "bootable {}", yes_no(partition.bootable));
        }
        block::PartitionTableKind::Gpt => {
            let _ = writeln!(text, "gpt-index {}", partition.gpt_index);
            let _ = writeln!(text, "gpt-type-guid {}", format_guid(&partition.gpt_type_guid));
            let _ = writeln!(
                text,
                "gpt-partition-guid {}",
                format_guid(&partition.gpt_partition_guid)
            );
        }
    }
    text
}

fn count_device_partitions(device_id: u16) -> usize {
    let mut count = 0usize;
    let mut index = 0usize;
    while index < block::partition_count() {
        if let Some(partition) = block::partition(index) {
            if partition.device_id == device_id {
                count += 1;
            }
        }
        index += 1;
    }
    count
}

fn dynamic_block_node_count() -> usize {
    if !block::is_initialized() {
        return 0;
    }

    let device_count = block::device_count();
    let partition_count = block::partition_count();
    device_count
        .saturating_mul(3)
        .saturating_add(partition_count.saturating_mul(3))
}

fn append_block_nodes(text: &mut String) {
    let mut device_index = 0usize;
    while device_index < block::device_count() {
        if let Some(device) = block::device(device_index) {
            let _ = writeln!(text, "{}", format_sd_disk_name(device_index));
            let _ = writeln!(text, "{}", format_nvme_disk_name(device_index));
            let _ = writeln!(text, "{}", format_nvm_disk_name(device_index));

            let mut partition_index = 1usize;
            let mut global_partition_index = 0usize;
            while global_partition_index < block::partition_count() {
                if let Some(partition) = block::partition(global_partition_index) {
                    if partition.device_id == device.id {
                        let _ = writeln!(
                            text,
                            "{}",
                            format_sd_partition_name(device_index, partition_index)
                        );
                        let _ = writeln!(
                            text,
                            "{}",
                            format_nvme_partition_name(device_index, partition_index)
                        );
                        let _ = writeln!(
                            text,
                            "{}",
                            format_nvm_partition_name(device_index, partition_index)
                        );
                        partition_index += 1;
                    }
                }
                global_partition_index += 1;
            }
        }
        device_index += 1;
    }
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

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
