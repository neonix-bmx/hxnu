use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

use crate::mm::frame;

const HEAP_PAGES: usize = 32;

struct BumpAllocator {
    start: usize,
    end: usize,
    next: usize,
    allocations: usize,
    initialized: bool,
}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            next: 0,
            allocations: 0,
            initialized: false,
        }
    }

    fn initialize(&mut self, start: usize, size: usize) -> Result<HeapStats, HeapInitError> {
        if self.initialized {
            return Err(HeapInitError::AlreadyInitialized);
        }

        if size == 0 {
            return Err(HeapInitError::OutOfFrames);
        }

        self.start = start;
        self.end = start.saturating_add(size);
        self.next = start;
        self.allocations = 0;
        self.initialized = true;

        Ok(self.stats())
    }

    fn stats(&self) -> HeapStats {
        HeapStats {
            start: self.start as u64,
            size_bytes: self.end.saturating_sub(self.start) as u64,
            used_bytes: self.next.saturating_sub(self.start) as u64,
            allocation_count: self.allocations,
        }
    }
}

struct GlobalBumpAllocator(UnsafeCell<BumpAllocator>);

unsafe impl Sync for GlobalBumpAllocator {}

impl GlobalBumpAllocator {
    const fn new() -> Self {
        Self(UnsafeCell::new(BumpAllocator::new()))
    }

    fn get(&self) -> *mut BumpAllocator {
        self.0.get()
    }
}

#[derive(Copy, Clone)]
pub struct HeapStats {
    pub start: u64,
    pub size_bytes: u64,
    pub used_bytes: u64,
    pub allocation_count: usize,
}

#[derive(Copy, Clone, Debug)]
pub enum HeapInitError {
    AlreadyInitialized,
    OutOfFrames,
}

#[global_allocator]
static GLOBAL_ALLOCATOR: LockedBumpAllocator = LockedBumpAllocator;

static HEAP_ALLOCATOR: GlobalBumpAllocator = GlobalBumpAllocator::new();

struct LockedBumpAllocator;

unsafe impl GlobalAlloc for LockedBumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let allocator = unsafe { &mut *HEAP_ALLOCATOR.get() };
        if !allocator.initialized {
            return null_mut();
        }

        let alloc_start = align_up(allocator.next, layout.align());
        let alloc_end = alloc_start.saturating_add(layout.size());
        if alloc_end > allocator.end {
            return null_mut();
        }

        allocator.next = alloc_end;
        allocator.allocations = allocator.allocations.saturating_add(1);
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let allocator = unsafe { &mut *HEAP_ALLOCATOR.get() };
        if allocator.allocations > 0 {
            allocator.allocations -= 1;
            if allocator.allocations == 0 {
                allocator.next = allocator.start;
            }
        }
    }
}

pub fn initialize(hhdm_offset: u64) -> Result<HeapStats, HeapInitError> {
    let heap_frame = frame::allocate_contiguous_frames(HEAP_PAGES as u64).ok_or(HeapInitError::OutOfFrames)?;
    let heap_start = hhdm_offset.saturating_add(heap_frame.start_address()) as usize;
    let heap_size = HEAP_PAGES * frame::PAGE_SIZE as usize;
    unsafe { (*HEAP_ALLOCATOR.get()).initialize(heap_start, heap_size) }
}

pub fn stats() -> HeapStats {
    unsafe { (*HEAP_ALLOCATOR.get()).stats() }
}

const fn align_up(value: usize, alignment: usize) -> usize {
    if alignment == 0 {
        return value;
    }
    value.saturating_add(alignment - 1) & !(alignment - 1)
}
