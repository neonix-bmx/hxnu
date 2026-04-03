#![allow(dead_code)]

use core::cell::UnsafeCell;

use crate::initrd;

const MAX_BLOCK_DEVICES: usize = 8;
const MAX_PARTITIONS: usize = 32;
pub const SECTOR_BYTES: usize = 512;
const MBR_SIGNATURE_OFFSET: usize = 510;
const MBR_PARTITION_TABLE_OFFSET: usize = 446;
const MBR_PARTITION_ENTRY_BYTES: usize = 16;
const MBR_PARTITION_ENTRY_COUNT: usize = 4;

#[derive(Copy, Clone)]
pub enum BlockDeviceKind {
    InitrdArchive,
}

impl BlockDeviceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InitrdArchive => "initrd-archive",
        }
    }
}

#[derive(Copy, Clone)]
pub struct BlockDeviceInfo {
    pub id: u16,
    pub kind: BlockDeviceKind,
    pub name: &'static str,
    pub read_only: bool,
    pub sector_size: usize,
    pub sector_count: u64,
    pub size_bytes: u64,
}

#[derive(Copy, Clone)]
pub struct PartitionInfo {
    pub id: u16,
    pub device_id: u16,
    pub mbr_index: u8,
    pub partition_type: u8,
    pub bootable: bool,
    pub start_lba: u64,
    pub sector_count: u64,
}

#[derive(Copy, Clone)]
pub struct BlockSummary {
    pub device_count: usize,
    pub partition_count: usize,
    pub total_bytes: u64,
    pub mbr_device_count: u64,
}

#[derive(Copy, Clone, Debug)]
pub enum BlockInitError {
    AlreadyInitialized,
}

impl BlockInitError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyInitialized => "block layer is already initialized",
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BlockReadError {
    NotInitialized,
    DeviceNotFound,
    InvalidBufferLength,
    SectorCountOverflow,
    OutOfRange,
}

impl BlockReadError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "block layer is not initialized",
            Self::DeviceNotFound => "block device is not found",
            Self::InvalidBufferLength => "output buffer length does not match sector request",
            Self::SectorCountOverflow => "sector count multiplication overflow",
            Self::OutOfRange => "block read is out of range",
        }
    }
}

#[derive(Copy, Clone)]
pub struct BlockStats {
    pub read_requests: u64,
    pub read_sectors: u64,
    pub read_bytes: u64,
    pub read_failures: u64,
}

impl BlockStats {
    const fn new() -> Self {
        Self {
            read_requests: 0,
            read_sectors: 0,
            read_bytes: 0,
            read_failures: 0,
        }
    }
}

#[derive(Copy, Clone)]
struct BlockDeviceSlot {
    present: bool,
    info: BlockDeviceInfo,
    bytes: &'static [u8],
}

impl BlockDeviceSlot {
    const fn empty() -> Self {
        Self {
            present: false,
            info: BlockDeviceInfo {
                id: 0,
                kind: BlockDeviceKind::InitrdArchive,
                name: "",
                read_only: true,
                sector_size: SECTOR_BYTES,
                sector_count: 0,
                size_bytes: 0,
            },
            bytes: &[],
        }
    }
}

#[derive(Copy, Clone)]
struct PartitionSlot {
    present: bool,
    info: PartitionInfo,
}

impl PartitionSlot {
    const fn empty() -> Self {
        Self {
            present: false,
            info: PartitionInfo {
                id: 0,
                device_id: 0,
                mbr_index: 0,
                partition_type: 0,
                bootable: false,
                start_lba: 0,
                sector_count: 0,
            },
        }
    }
}

struct BlockState {
    initialized: bool,
    device_count: usize,
    partition_count: usize,
    total_bytes: u64,
    mbr_device_count: u64,
    devices: [BlockDeviceSlot; MAX_BLOCK_DEVICES],
    partitions: [PartitionSlot; MAX_PARTITIONS],
    stats: BlockStats,
}

impl BlockState {
    const fn new() -> Self {
        Self {
            initialized: false,
            device_count: 0,
            partition_count: 0,
            total_bytes: 0,
            mbr_device_count: 0,
            devices: [BlockDeviceSlot::empty(); MAX_BLOCK_DEVICES],
            partitions: [PartitionSlot::empty(); MAX_PARTITIONS],
            stats: BlockStats::new(),
        }
    }

    fn initialize(&mut self) -> Result<BlockSummary, BlockInitError> {
        if self.initialized {
            return Err(BlockInitError::AlreadyInitialized);
        }
        self.initialized = true;

        if let Some(bytes) = initrd::archive_bytes() {
            self.register_device(BlockDeviceKind::InitrdArchive, "initrd0", true, bytes);
        }

        self.discover_mbr_partitions();
        Ok(self.summary())
    }

    fn summary(&self) -> BlockSummary {
        BlockSummary {
            device_count: self.device_count,
            partition_count: self.partition_count,
            total_bytes: self.total_bytes,
            mbr_device_count: self.mbr_device_count,
        }
    }

    fn stats(&self) -> BlockStats {
        self.stats
    }

    fn device(&self, index: usize) -> Option<BlockDeviceInfo> {
        if index >= self.device_count {
            return None;
        }
        let slot = self.devices[index];
        if !slot.present {
            return None;
        }
        Some(slot.info)
    }

    fn partition(&self, index: usize) -> Option<PartitionInfo> {
        if index >= self.partition_count {
            return None;
        }
        let slot = self.partitions[index];
        if !slot.present {
            return None;
        }
        Some(slot.info)
    }

    fn read(
        &mut self,
        device_id: u16,
        lba: u64,
        sector_count: usize,
        out: &mut [u8],
    ) -> Result<(), BlockReadError> {
        if !self.initialized {
            return Err(BlockReadError::NotInitialized);
        }

        let expected_len = sector_count
            .checked_mul(SECTOR_BYTES)
            .ok_or(BlockReadError::SectorCountOverflow)?;
        if out.len() != expected_len {
            return Err(BlockReadError::InvalidBufferLength);
        }

        self.stats.read_requests = self.stats.read_requests.saturating_add(1);
        let result = self.read_from_device(device_id, lba, sector_count, out);
        match result {
            Ok(()) => {
                self.stats.read_sectors = self.stats.read_sectors.saturating_add(sector_count as u64);
                self.stats.read_bytes = self.stats.read_bytes.saturating_add(expected_len as u64);
                Ok(())
            }
            Err(error) => {
                self.stats.read_failures = self.stats.read_failures.saturating_add(1);
                Err(error)
            }
        }
    }

    fn register_device(
        &mut self,
        kind: BlockDeviceKind,
        name: &'static str,
        read_only: bool,
        bytes: &'static [u8],
    ) {
        if self.device_count >= MAX_BLOCK_DEVICES {
            return;
        }

        let id = self.device_count as u16;
        let size_bytes = bytes.len() as u64;
        let sector_count = size_bytes.div_ceil(SECTOR_BYTES as u64);
        self.devices[self.device_count] = BlockDeviceSlot {
            present: true,
            info: BlockDeviceInfo {
                id,
                kind,
                name,
                read_only,
                sector_size: SECTOR_BYTES,
                sector_count,
                size_bytes,
            },
            bytes,
        };
        self.device_count += 1;
        self.total_bytes = self.total_bytes.saturating_add(size_bytes);
    }

    fn discover_mbr_partitions(&mut self) {
        let mut sector = [0u8; SECTOR_BYTES];
        let mut device_index = 0usize;
        while device_index < self.device_count {
            let device = self.devices[device_index];
            if !device.present {
                device_index += 1;
                continue;
            }

            if self
                .read_bytes_raw(device.info.id, 0, 1, &mut sector)
                .is_err()
            {
                device_index += 1;
                continue;
            }

            if sector[MBR_SIGNATURE_OFFSET] != 0x55 || sector[MBR_SIGNATURE_OFFSET + 1] != 0xAA {
                device_index += 1;
                continue;
            }
            self.mbr_device_count = self.mbr_device_count.saturating_add(1);

            let mut entry_index = 0usize;
            while entry_index < MBR_PARTITION_ENTRY_COUNT {
                let base = MBR_PARTITION_TABLE_OFFSET + (entry_index * MBR_PARTITION_ENTRY_BYTES);
                let bootable = sector[base] == 0x80;
                let partition_type = sector[base + 4];
                let start_lba = u32::from_le_bytes([
                    sector[base + 8],
                    sector[base + 9],
                    sector[base + 10],
                    sector[base + 11],
                ]) as u64;
                let sector_count = u32::from_le_bytes([
                    sector[base + 12],
                    sector[base + 13],
                    sector[base + 14],
                    sector[base + 15],
                ]) as u64;

                if partition_type != 0 && sector_count != 0 {
                    self.register_partition(PartitionInfo {
                        id: self.partition_count as u16,
                        device_id: device.info.id,
                        mbr_index: (entry_index + 1) as u8,
                        partition_type,
                        bootable,
                        start_lba,
                        sector_count,
                    });
                }

                entry_index += 1;
            }

            device_index += 1;
        }
    }

    fn register_partition(&mut self, info: PartitionInfo) {
        if self.partition_count >= MAX_PARTITIONS {
            return;
        }
        self.partitions[self.partition_count] = PartitionSlot {
            present: true,
            info,
        };
        self.partition_count += 1;
    }

    fn read_from_device(
        &self,
        device_id: u16,
        lba: u64,
        sector_count: usize,
        out: &mut [u8],
    ) -> Result<(), BlockReadError> {
        self.read_bytes_raw(device_id, lba, sector_count, out)
    }

    fn read_bytes_raw(
        &self,
        device_id: u16,
        lba: u64,
        sector_count: usize,
        out: &mut [u8],
    ) -> Result<(), BlockReadError> {
        let slot_idx = self.find_device_slot(device_id).ok_or(BlockReadError::DeviceNotFound)?;
        let bytes = self.devices[slot_idx].bytes;

        let mut sector_index = 0usize;
        while sector_index < sector_count {
            let sector_lba = lba
                .checked_add(sector_index as u64)
                .ok_or(BlockReadError::OutOfRange)?;
            let byte_offset = sector_lba
                .checked_mul(SECTOR_BYTES as u64)
                .ok_or(BlockReadError::OutOfRange)? as usize;

            if byte_offset >= bytes.len() {
                return Err(BlockReadError::OutOfRange);
            }

            let out_offset = sector_index
                .checked_mul(SECTOR_BYTES)
                .ok_or(BlockReadError::SectorCountOverflow)?;
            let out_sector = &mut out[out_offset..out_offset + SECTOR_BYTES];
            out_sector.fill(0);

            let copy_end = byte_offset.saturating_add(SECTOR_BYTES).min(bytes.len());
            let copy_len = copy_end.saturating_sub(byte_offset);
            out_sector[..copy_len].copy_from_slice(&bytes[byte_offset..byte_offset + copy_len]);

            sector_index += 1;
        }
        Ok(())
    }

    fn find_device_slot(&self, device_id: u16) -> Option<usize> {
        let mut index = 0usize;
        while index < self.device_count {
            let slot = self.devices[index];
            if slot.present && slot.info.id == device_id {
                return Some(index);
            }
            index += 1;
        }
        None
    }
}

struct GlobalBlock(UnsafeCell<BlockState>);

unsafe impl Sync for GlobalBlock {}

impl GlobalBlock {
    const fn new() -> Self {
        Self(UnsafeCell::new(BlockState::new()))
    }

    fn get(&self) -> *mut BlockState {
        self.0.get()
    }
}

static BLOCK: GlobalBlock = GlobalBlock::new();

pub fn initialize() -> Result<BlockSummary, BlockInitError> {
    unsafe { (*BLOCK.get()).initialize() }
}

pub fn summary() -> BlockSummary {
    unsafe { (*BLOCK.get()).summary() }
}

pub fn stats() -> BlockStats {
    unsafe { (*BLOCK.get()).stats() }
}

pub fn is_initialized() -> bool {
    unsafe { (*BLOCK.get()).initialized }
}

pub fn device_count() -> usize {
    unsafe { (*BLOCK.get()).device_count }
}

pub fn partition_count() -> usize {
    unsafe { (*BLOCK.get()).partition_count }
}

pub fn device(index: usize) -> Option<BlockDeviceInfo> {
    unsafe { (*BLOCK.get()).device(index) }
}

pub fn partition(index: usize) -> Option<PartitionInfo> {
    unsafe { (*BLOCK.get()).partition(index) }
}

pub fn read(device_id: u16, lba: u64, sector_count: usize, out: &mut [u8]) -> Result<(), BlockReadError> {
    unsafe { (*BLOCK.get()).read(device_id, lba, sector_count, out) }
}
