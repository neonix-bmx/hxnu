use core::cell::UnsafeCell;

use crate::limine::MemoryMap;

pub const PAGE_SIZE: u64 = 4096;

const MAX_USABLE_REGIONS: usize = 64;
const BOOTSTRAP_RESERVED_END: u64 = 0x0010_0000;

#[derive(Copy, Clone)]
struct FrameRegion {
    start: u64,
    end: u64,
    next: u64,
}

impl FrameRegion {
    const fn empty() -> Self {
        Self {
            start: 0,
            end: 0,
            next: 0,
        }
    }

    fn size(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    fn remaining(self) -> u64 {
        self.end.saturating_sub(self.next)
    }
}

#[derive(Copy, Clone)]
pub struct PhysicalFrame {
    start_address: u64,
}

impl PhysicalFrame {
    pub fn start_address(self) -> u64 {
        self.start_address
    }
}

#[derive(Copy, Clone)]
pub struct FrameAllocatorStats {
    pub usable_regions: usize,
    pub total_bytes: u64,
    pub allocatable_bytes: u64,
    pub allocated_frames: u64,
}

#[derive(Copy, Clone, Debug)]
pub enum FrameAllocatorInitError {
    AlreadyInitialized,
    NoUsableRegions,
    TooManyRegions,
}

struct FrameAllocator {
    initialized: bool,
    region_count: usize,
    total_bytes: u64,
    allocatable_bytes: u64,
    allocated_frames: u64,
    regions: [FrameRegion; MAX_USABLE_REGIONS],
}

impl FrameAllocator {
    const fn new() -> Self {
        Self {
            initialized: false,
            region_count: 0,
            total_bytes: 0,
            allocatable_bytes: 0,
            allocated_frames: 0,
            regions: [FrameRegion::empty(); MAX_USABLE_REGIONS],
        }
    }

    fn initialize(&mut self, memory_map: &MemoryMap) -> Result<FrameAllocatorStats, FrameAllocatorInitError> {
        if self.initialized {
            return Err(FrameAllocatorInitError::AlreadyInitialized);
        }

        for entry in memory_map.iter() {
            if !entry.is_usable() {
                continue;
            }

            let original_end = entry.base.saturating_add(entry.length);
            self.total_bytes = self.total_bytes.saturating_add(entry.length);

            let region_start = align_up(entry.base.max(BOOTSTRAP_RESERVED_END), PAGE_SIZE);
            let region_end = align_down(original_end, PAGE_SIZE);

            if region_end <= region_start {
                continue;
            }

            if self.region_count >= MAX_USABLE_REGIONS {
                return Err(FrameAllocatorInitError::TooManyRegions);
            }

            let region = FrameRegion {
                start: region_start,
                end: region_end,
                next: region_start,
            };

            self.allocatable_bytes = self.allocatable_bytes.saturating_add(region.size());
            self.regions[self.region_count] = region;
            self.region_count += 1;
        }

        if self.region_count == 0 {
            return Err(FrameAllocatorInitError::NoUsableRegions);
        }

        self.initialized = true;
        Ok(self.stats())
    }

    fn allocate(&mut self) -> Option<PhysicalFrame> {
        if !self.initialized {
            return None;
        }

        for region in &mut self.regions[..self.region_count] {
            if region.remaining() < PAGE_SIZE {
                continue;
            }

            let frame = PhysicalFrame {
                start_address: region.next,
            };
            region.next = region.next.saturating_add(PAGE_SIZE);
            self.allocated_frames = self.allocated_frames.saturating_add(1);
            return Some(frame);
        }

        None
    }

    fn allocate_contiguous(&mut self, count: u64) -> Option<PhysicalFrame> {
        if !self.initialized || count == 0 {
            return None;
        }

        let requested_bytes = count.saturating_mul(PAGE_SIZE);
        for region in &mut self.regions[..self.region_count] {
            if region.remaining() < requested_bytes {
                continue;
            }

            let frame = PhysicalFrame {
                start_address: region.next,
            };
            region.next = region.next.saturating_add(requested_bytes);
            self.allocated_frames = self.allocated_frames.saturating_add(count);
            return Some(frame);
        }

        None
    }

    fn stats(&self) -> FrameAllocatorStats {
        FrameAllocatorStats {
            usable_regions: self.region_count,
            total_bytes: self.total_bytes,
            allocatable_bytes: self.allocatable_bytes,
            allocated_frames: self.allocated_frames,
        }
    }
}

struct GlobalFrameAllocator(UnsafeCell<FrameAllocator>);

unsafe impl Sync for GlobalFrameAllocator {}

impl GlobalFrameAllocator {
    const fn new() -> Self {
        Self(UnsafeCell::new(FrameAllocator::new()))
    }

    fn get(&self) -> *mut FrameAllocator {
        self.0.get()
    }
}

static FRAME_ALLOCATOR: GlobalFrameAllocator = GlobalFrameAllocator::new();

pub fn initialize(memory_map: &MemoryMap) -> Result<FrameAllocatorStats, FrameAllocatorInitError> {
    unsafe { (*FRAME_ALLOCATOR.get()).initialize(memory_map) }
}

pub fn allocate_frame() -> Option<PhysicalFrame> {
    unsafe { (*FRAME_ALLOCATOR.get()).allocate() }
}

pub fn allocate_contiguous_frames(count: u64) -> Option<PhysicalFrame> {
    unsafe { (*FRAME_ALLOCATOR.get()).allocate_contiguous(count) }
}

pub fn stats() -> FrameAllocatorStats {
    unsafe { (*FRAME_ALLOCATOR.get()).stats() }
}

const fn align_up(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    value.saturating_add(alignment - 1) & !(alignment - 1)
}

const fn align_down(value: u64, alignment: u64) -> u64 {
    if alignment == 0 {
        return value;
    }
    value & !(alignment - 1)
}
