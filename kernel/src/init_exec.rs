use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt::Write;

use crate::vfs;
use crate::vfs::{ExecutableFormat, VmMapImageEntry};

struct GlobalInitExec(UnsafeCell<InitExecState>);

unsafe impl Sync for GlobalInitExec {}

impl GlobalInitExec {
    const fn new() -> Self {
        Self(UnsafeCell::new(InitExecState::new()))
    }

    fn get(&self) -> *mut InitExecState {
        self.0.get()
    }
}

static INIT_EXEC: GlobalInitExec = GlobalInitExec::new();

struct InitExecState {
    activation: Option<ActivatedInitImage>,
    last_error: Option<InitExecActivateError>,
}

impl InitExecState {
    const fn new() -> Self {
        Self {
            activation: None,
            last_error: None,
        }
    }
}

struct ActivatedInitImage {
    path: String,
    format: ExecutableFormat,
    image_type: u16,
    machine: u16,
    entry_point: u64,
    vm_start: u64,
    vm_end: u64,
    total_bytes: u64,
    zero_fill_bytes: u64,
    entry_segment_index: usize,
    entry_segment_map_offset: u64,
    segments: Vec<ActivatedSegment>,
}

struct ActivatedSegment {
    index: usize,
    virtual_start: u64,
    virtual_end: u64,
    map_start: u64,
    map_end: u64,
    file_bytes: u64,
    memory_bytes: u64,
    readable: bool,
    writable: bool,
    executable: bool,
    bytes: Vec<u8>,
}

#[derive(Copy, Clone)]
pub struct InitExecSummary {
    pub armed: bool,
    pub format: ExecutableFormat,
    pub image_type: u16,
    pub machine: u16,
    pub entry_point: u64,
    pub segment_count: usize,
    pub total_bytes: u64,
    pub zero_fill_bytes: u64,
    pub vm_start: u64,
    pub vm_end: u64,
    pub entry_segment_index: usize,
    pub entry_segment_map_offset: u64,
}

#[derive(Copy, Clone)]
pub enum InitExecActivateError {
    Load(vfs::ExecutableLoadPrepError),
    UnsupportedFormat,
    MissingEntryPoint,
    MissingImageType,
    MissingMachine,
    NoLoadSegments,
    NoExecutableSegments,
    EntryOutsideExecutableSegments,
    InvalidSegmentMap,
}

impl InitExecActivateError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Load(error) => error.as_str(),
            Self::UnsupportedFormat => "init executable format is not ELF",
            Self::MissingEntryPoint => "init executable has no entry point",
            Self::MissingImageType => "init executable is missing ELF image type",
            Self::MissingMachine => "init executable is missing ELF machine id",
            Self::NoLoadSegments => "init executable has no loadable segments",
            Self::NoExecutableSegments => "init executable has no executable load segment",
            Self::EntryOutsideExecutableSegments => "entry point is outside executable segments",
            Self::InvalidSegmentMap => "materialized segment map is invalid",
        }
    }
}

pub fn activate_init_handoff() -> Result<InitExecSummary, InitExecActivateError> {
    let result = (|| {
        let image = vfs::materialize_init_image().map_err(InitExecActivateError::Load)?;
        build_activation(image)
    })();

    let state = state_mut();
    match result {
        Ok(activation) => {
            let summary = activation_summary(&activation);
            state.activation = Some(activation);
            state.last_error = None;
            Ok(summary)
        }
        Err(error) => {
            state.activation = None;
            state.last_error = Some(error);
            Err(error)
        }
    }
}

pub fn render_status() -> String {
    let state = state_ref();
    let mut text = String::new();

    match state.activation.as_ref() {
        Some(activation) => {
            let summary = activation_summary(activation);
            let _ = writeln!(text, "armed {}", yes_no(summary.armed));
            let _ = writeln!(text, "path {}", activation.path);
            let _ = writeln!(text, "format {}", summary.format.as_str());
            let _ = writeln!(text, "machine {:#06x}", summary.machine);
            let _ = writeln!(text, "image_type {:#06x}", summary.image_type);
            let _ = writeln!(text, "entry_point {:#018x}", summary.entry_point);
            let _ = writeln!(
                text,
                "vm_range {:#018x}..{:#018x}",
                summary.vm_start,
                summary.vm_end
            );
            let _ = writeln!(text, "segments {}", summary.segment_count);
            let _ = writeln!(text, "bytes {}", summary.total_bytes);
            let _ = writeln!(text, "zero_fill {}", summary.zero_fill_bytes);
            let _ = writeln!(text, "entry_segment {}", summary.entry_segment_index);
            let _ = writeln!(text, "entry_offset {}", summary.entry_segment_map_offset);
            if let Some(segment) = activation.segments.first() {
                let _ = writeln!(
                    text,
                    "segment0 idx={} vaddr={:#018x}..{:#018x} map={:#018x}..{:#018x} file={} mem={} perms={}{}{} bytes={}",
                    segment.index,
                    segment.virtual_start,
                    segment.virtual_end,
                    segment.map_start,
                    segment.map_end,
                    segment.file_bytes,
                    segment.memory_bytes,
                    if segment.readable { 'r' } else { '-' },
                    if segment.writable { 'w' } else { '-' },
                    if segment.executable { 'x' } else { '-' },
                    segment.bytes.len(),
                );
            }
        }
        None => {
            let _ = writeln!(text, "armed no");
        }
    }

    let last_error = state.last_error.map(|error| error.as_str()).unwrap_or("<none>");
    let _ = writeln!(text, "last_error {}", last_error);
    text
}

fn build_activation(image: vfs::ExecutableLoadImage) -> Result<ActivatedInitImage, InitExecActivateError> {
    if image.format != ExecutableFormat::Elf {
        return Err(InitExecActivateError::UnsupportedFormat);
    }

    let entry_point = image.entry_point.ok_or(InitExecActivateError::MissingEntryPoint)?;
    let image_type = image.image_type.ok_or(InitExecActivateError::MissingImageType)?;
    let machine = image.machine.ok_or(InitExecActivateError::MissingMachine)?;
    if image.vm_map_images.is_empty() {
        return Err(InitExecActivateError::NoLoadSegments);
    }

    let mut vm_start = u64::MAX;
    let mut vm_end = 0u64;
    let mut has_executable_segment = false;
    let mut entry_segment_index = None;
    let mut entry_segment_map_offset = 0u64;
    let mut segments = Vec::with_capacity(image.vm_map_images.len());

    for segment in image.vm_map_images.into_iter() {
        let expected_len = segment
            .map_end
            .checked_sub(segment.map_start)
            .ok_or(InitExecActivateError::InvalidSegmentMap)?;
        let actual_len = u64::try_from(segment.bytes.len()).map_err(|_| InitExecActivateError::InvalidSegmentMap)?;
        if expected_len != actual_len {
            return Err(InitExecActivateError::InvalidSegmentMap);
        }

        vm_start = vm_start.min(segment.map_start);
        vm_end = vm_end.max(segment.map_end);

        if segment.executable {
            has_executable_segment = true;
            if entry_point >= segment.virtual_start && entry_point < segment.virtual_end {
                entry_segment_index = Some(segment.index);
                entry_segment_map_offset = entry_point
                    .checked_sub(segment.map_start)
                    .ok_or(InitExecActivateError::InvalidSegmentMap)?;
            }
        }

        segments.push(to_activated_segment(segment));
    }

    if !has_executable_segment {
        return Err(InitExecActivateError::NoExecutableSegments);
    }
    let entry_segment_index = entry_segment_index.ok_or(InitExecActivateError::EntryOutsideExecutableSegments)?;

    Ok(ActivatedInitImage {
        path: image.path,
        format: image.format,
        image_type,
        machine,
        entry_point,
        vm_start,
        vm_end,
        total_bytes: image.vm_map_total_bytes,
        zero_fill_bytes: image.vm_map_zero_fill_bytes,
        entry_segment_index,
        entry_segment_map_offset,
        segments,
    })
}

fn to_activated_segment(segment: VmMapImageEntry) -> ActivatedSegment {
    ActivatedSegment {
        index: segment.index,
        virtual_start: segment.virtual_start,
        virtual_end: segment.virtual_end,
        map_start: segment.map_start,
        map_end: segment.map_end,
        file_bytes: segment.file_bytes,
        memory_bytes: segment.memory_bytes,
        readable: segment.readable,
        writable: segment.writable,
        executable: segment.executable,
        bytes: segment.bytes,
    }
}

fn activation_summary(activation: &ActivatedInitImage) -> InitExecSummary {
    InitExecSummary {
        armed: true,
        format: activation.format,
        image_type: activation.image_type,
        machine: activation.machine,
        entry_point: activation.entry_point,
        segment_count: activation.segments.len(),
        total_bytes: activation.total_bytes,
        zero_fill_bytes: activation.zero_fill_bytes,
        vm_start: activation.vm_start,
        vm_end: activation.vm_end,
        entry_segment_index: activation.entry_segment_index,
        entry_segment_map_offset: activation.entry_segment_map_offset,
    }
}

fn state_ref() -> &'static InitExecState {
    unsafe { &*INIT_EXEC.get() }
}

fn state_mut() -> &'static mut InitExecState {
    unsafe { &mut *INIT_EXEC.get() }
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
