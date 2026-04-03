#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::block;

const FAT_PATH_ROOT: &str = "/fat";
const DIRECTORY_ENTRY_BYTES: usize = 32;
const FAT32_EOC_MIN: u32 = 0x0fff_fff8;
const FAT32_BAD_CLUSTER: u32 = 0x0fff_fff7;
const MAX_FAT32_ROOT_CHAIN_STEPS: usize = 1024;

struct GlobalFat(UnsafeCell<Option<FatState>>);

unsafe impl Sync for GlobalFat {}

impl GlobalFat {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<FatState> {
        self.0.get()
    }
}

static FAT: GlobalFat = GlobalFat::new();

struct FatState {
    summary: FatSummary,
    root_entries: Vec<FatRootEntry>,
}

#[derive(Clone)]
struct FatRootEntry {
    name: String,
    kind: FatNodeKind,
    size: usize,
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FatType {
    Fat16,
    Fat32,
}

impl FatType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fat16 => "fat16",
            Self::Fat32 => "fat32",
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum FatNodeKind {
    Directory,
    File,
}

#[derive(Copy, Clone)]
pub struct FatNodeInfo {
    pub kind: FatNodeKind,
    pub size: usize,
}

#[derive(Copy, Clone)]
pub struct FatSummary {
    pub mounted: bool,
    pub partition_id: Option<u16>,
    pub device_id: Option<u16>,
    pub partition_table: Option<block::PartitionTableKind>,
    pub fat_type: Option<FatType>,
    pub root_entry_count: usize,
    pub directory_count: usize,
}

impl FatSummary {
    const fn offline() -> Self {
        Self {
            mounted: false,
            partition_id: None,
            device_id: None,
            partition_table: None,
            fat_type: None,
            root_entry_count: 0,
            directory_count: 0,
        }
    }
}

#[derive(Copy, Clone)]
pub enum FatError {
    AlreadyInitialized,
    BlockUnavailable,
    NoFatPartition,
}

impl FatError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "fat is already initialized",
            Self::BlockUnavailable => "block layer is unavailable",
            Self::NoFatPartition => "no FAT16/32 partition found",
        }
    }
}

pub fn initialize() -> Result<FatSummary, FatError> {
    let slot = unsafe { &mut *FAT.get() };
    if slot.is_some() {
        return Err(FatError::AlreadyInitialized);
    }
    if !block::is_initialized() {
        return Err(FatError::BlockUnavailable);
    }

    let mut index = 0usize;
    while index < block::partition_count() {
        if let Some(partition) = block::partition(index) {
            if let Some(state) = try_mount_partition(partition) {
                let summary = state.summary;
                *slot = Some(state);
                return Ok(summary);
            }
        }
        index += 1;
    }

    Err(FatError::NoFatPartition)
}

pub fn is_initialized() -> bool {
    unsafe { (&*FAT.get()).is_some() }
}

pub fn summary() -> FatSummary {
    let Some(state) = (unsafe { (&*FAT.get()).as_ref() }) else {
        return FatSummary::offline();
    };
    state.summary
}

pub fn node_kind(path: &str) -> Option<FatNodeKind> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let normalized = normalize_fat_path(path)?;
    if normalized == FAT_PATH_ROOT {
        return Some(FatNodeKind::Directory);
    }

    let name = normalized.strip_prefix("/fat/")?;
    state
        .root_entries
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| entry.kind)
}

pub fn node_info(path: &str) -> Option<FatNodeInfo> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let normalized = normalize_fat_path(path)?;
    if normalized == FAT_PATH_ROOT {
        return Some(FatNodeInfo {
            kind: FatNodeKind::Directory,
            size: render_root_entries(&state.root_entries).len(),
        });
    }

    let name = normalized.strip_prefix("/fat/")?;
    state
        .root_entries
        .iter()
        .find(|entry| entry.name == name)
        .map(|entry| FatNodeInfo {
            kind: entry.kind,
            size: entry.size,
        })
}

pub fn read(path: &str) -> Option<String> {
    let state = unsafe { (&*FAT.get()).as_ref()? };
    let normalized = normalize_fat_path(path)?;
    if normalized == FAT_PATH_ROOT {
        return Some(render_root_entries(&state.root_entries));
    }
    None
}

fn try_mount_partition(partition: block::PartitionInfo) -> Option<FatState> {
    let mut bpb_sector = [0u8; block::SECTOR_BYTES];
    if block::read(partition.device_id, partition.start_lba, 1, &mut bpb_sector).is_err() {
        return None;
    }
    if bpb_sector[510] != 0x55 || bpb_sector[511] != 0xAA {
        return None;
    }

    let bpb = parse_bpb(&bpb_sector)?;
    let (fat_type, root_entries) = match bpb.fat_type {
        FatType::Fat16 => {
            let entries = read_fat16_root_entries(partition, &bpb)?;
            (FatType::Fat16, entries)
        }
        FatType::Fat32 => {
            let entries = read_fat32_root_entries(partition, &bpb)?;
            (FatType::Fat32, entries)
        }
    };

    let directory_count = 1 + root_entries
        .iter()
        .filter(|entry| entry.kind == FatNodeKind::Directory)
        .count();

    Some(FatState {
        summary: FatSummary {
            mounted: true,
            partition_id: Some(partition.id),
            device_id: Some(partition.device_id),
            partition_table: Some(partition.table_kind),
            fat_type: Some(fat_type),
            root_entry_count: root_entries.len(),
            directory_count,
        },
        root_entries,
    })
}

#[derive(Copy, Clone)]
struct BpbLayout {
    fat_type: FatType,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    fat_count: u8,
    sectors_per_fat: u32,
    root_dir_entries: u16,
    root_dir_sectors: u32,
    fat_start_lba_offset: u32,
    root_dir_start_lba_offset: u32,
    first_data_sector_offset: u32,
    root_dir_first_cluster: u32,
}

fn parse_bpb(sector: &[u8; block::SECTOR_BYTES]) -> Option<BpbLayout> {
    let bytes_per_sector = read_u16_le(sector, 11);
    if bytes_per_sector != block::SECTOR_BYTES as u16 {
        return None;
    }

    let sectors_per_cluster = sector[13];
    if sectors_per_cluster == 0 {
        return None;
    }

    let reserved_sectors = read_u16_le(sector, 14);
    if reserved_sectors == 0 {
        return None;
    }

    let fat_count = sector[16];
    if fat_count == 0 {
        return None;
    }

    let root_dir_entries = read_u16_le(sector, 17);
    let total_sectors_16 = read_u16_le(sector, 19) as u32;
    let sectors_per_fat_16 = read_u16_le(sector, 22) as u32;
    let total_sectors_32 = read_u32_le(sector, 32);
    let sectors_per_fat_32 = read_u32_le(sector, 36);
    let root_dir_first_cluster = read_u32_le(sector, 44);

    let total_sectors = if total_sectors_16 != 0 {
        total_sectors_16
    } else {
        total_sectors_32
    };
    if total_sectors == 0 {
        return None;
    }

    let sectors_per_fat = if sectors_per_fat_16 != 0 {
        sectors_per_fat_16
    } else {
        sectors_per_fat_32
    };
    if sectors_per_fat == 0 {
        return None;
    }

    let root_dir_sectors =
        ((u32::from(root_dir_entries) * 32) + (u32::from(bytes_per_sector) - 1)) / u32::from(bytes_per_sector);
    let first_data_sector =
        u32::from(reserved_sectors) + (u32::from(fat_count) * sectors_per_fat) + root_dir_sectors;
    if first_data_sector >= total_sectors {
        return None;
    }

    let data_sectors = total_sectors.saturating_sub(first_data_sector);
    let cluster_count = data_sectors / u32::from(sectors_per_cluster);
    let fat_type = if cluster_count < 4_085 {
        return None;
    } else if cluster_count < 65_525 {
        FatType::Fat16
    } else {
        FatType::Fat32
    };

    if fat_type == FatType::Fat32 && root_dir_first_cluster < 2 {
        return None;
    }

    Some(BpbLayout {
        fat_type,
        bytes_per_sector,
        sectors_per_cluster,
        reserved_sectors,
        fat_count,
        sectors_per_fat,
        root_dir_entries,
        root_dir_sectors,
        fat_start_lba_offset: u32::from(reserved_sectors),
        root_dir_start_lba_offset: u32::from(reserved_sectors) + (u32::from(fat_count) * sectors_per_fat),
        first_data_sector_offset: first_data_sector,
        root_dir_first_cluster,
    })
}

fn read_fat16_root_entries(partition: block::PartitionInfo, bpb: &BpbLayout) -> Option<Vec<FatRootEntry>> {
    let mut entries = Vec::new();
    let start_lba = partition.start_lba + u64::from(bpb.root_dir_start_lba_offset);
    let mut sector = [0u8; block::SECTOR_BYTES];
    let mut sector_offset = 0u32;
    while sector_offset < bpb.root_dir_sectors {
        if block::read(
            partition.device_id,
            start_lba + u64::from(sector_offset),
            1,
            &mut sector,
        )
        .is_err()
        {
            return None;
        }
        if parse_directory_sector(&sector, &mut entries) {
            break;
        }
        sector_offset += 1;
    }
    Some(entries)
}

fn read_fat32_root_entries(partition: block::PartitionInfo, bpb: &BpbLayout) -> Option<Vec<FatRootEntry>> {
    let mut entries = Vec::new();
    let mut sector = [0u8; block::SECTOR_BYTES];
    let mut cluster = bpb.root_dir_first_cluster;
    let mut visited = 0usize;

    while visited < MAX_FAT32_ROOT_CHAIN_STEPS {
        if cluster < 2 {
            break;
        }
        let cluster_lba = partition.start_lba
            + u64::from(bpb.first_data_sector_offset)
            + (u64::from(cluster - 2) * u64::from(bpb.sectors_per_cluster));

        let mut sec = 0u8;
        while sec < bpb.sectors_per_cluster {
            if block::read(partition.device_id, cluster_lba + u64::from(sec), 1, &mut sector).is_err() {
                return None;
            }
            if parse_directory_sector(&sector, &mut entries) {
                return Some(entries);
            }
            sec += 1;
        }

        let next = read_fat32_entry(partition, bpb, cluster)?;
        if next >= FAT32_EOC_MIN || next == FAT32_BAD_CLUSTER || next == 0 || next == cluster {
            break;
        }
        cluster = next;
        visited += 1;
    }

    Some(entries)
}

fn read_fat32_entry(partition: block::PartitionInfo, bpb: &BpbLayout, cluster: u32) -> Option<u32> {
    let fat_offset = u64::from(cluster) * 4;
    let fat_sector_lba = partition.start_lba + u64::from(bpb.fat_start_lba_offset) + (fat_offset / 512);
    let fat_sector_offset = (fat_offset % 512) as usize;
    let mut sector = [0u8; block::SECTOR_BYTES];
    if block::read(partition.device_id, fat_sector_lba, 1, &mut sector).is_err() {
        return None;
    }
    Some(read_u32_le(&sector, fat_sector_offset) & 0x0fff_ffff)
}

fn parse_directory_sector(sector: &[u8; block::SECTOR_BYTES], out: &mut Vec<FatRootEntry>) -> bool {
    let mut offset = 0usize;
    while offset + DIRECTORY_ENTRY_BYTES <= sector.len() {
        let first = sector[offset];
        if first == 0x00 {
            return true;
        }
        if first == 0xE5 {
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let attrs = sector[offset + 11];
        if attrs == 0x0F || (attrs & 0x08) != 0 {
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let Some(name) = parse_short_name(&sector[offset..offset + 11]) else {
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        };
        if name == "." || name == ".." {
            offset += DIRECTORY_ENTRY_BYTES;
            continue;
        }

        let kind = if (attrs & 0x10) != 0 {
            FatNodeKind::Directory
        } else {
            FatNodeKind::File
        };
        let size = read_u32_le(sector, offset + 28) as usize;
        out.push(FatRootEntry { name, kind, size });

        offset += DIRECTORY_ENTRY_BYTES;
    }
    false
}

fn parse_short_name(name: &[u8]) -> Option<String> {
    if name.len() != 11 {
        return None;
    }

    let base = parse_name_component(&name[..8])?;
    if base.is_empty() {
        return None;
    }
    let ext = parse_name_component(&name[8..])?;

    let mut full = String::new();
    full.push_str(&base);
    if !ext.is_empty() {
        full.push('.');
        full.push_str(&ext);
    }
    Some(full)
}

fn parse_name_component(bytes: &[u8]) -> Option<String> {
    let mut text = String::new();
    for byte in bytes {
        if *byte == b' ' || *byte == 0 {
            break;
        }
        if !(0x21..=0x7e).contains(byte) {
            return None;
        }
        text.push(*byte as char);
    }
    Some(text)
}

fn render_root_entries(entries: &[FatRootEntry]) -> String {
    let mut text = String::new();
    for entry in entries {
        let _ = writeln!(text, "{}", entry.name);
    }
    text
}

fn normalize_fat_path(path: &str) -> Option<String> {
    if path == FAT_PATH_ROOT || path == "/fat/" {
        return Some(String::from(FAT_PATH_ROOT));
    }
    if !path.starts_with("/fat/") {
        return None;
    }

    let trimmed = path.trim_end_matches('/');
    let name = trimmed.strip_prefix("/fat/")?;
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let mut normalized = String::from(FAT_PATH_ROOT);
    normalized.push('/');
    normalized.push_str(name);
    Some(normalized)
}

fn read_u16_le(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([input[offset], input[offset + 1]])
}

fn read_u32_le(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        input[offset],
        input[offset + 1],
        input[offset + 2],
        input[offset + 3],
    ])
}
