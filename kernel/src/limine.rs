use core::ffi::CStr;
use core::ptr;

#[repr(C)]
pub struct LimineMemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: *const *const LimineMemmapEntry,
}

#[repr(C)]
pub struct LimineMemmapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u64,
}

#[repr(C)]
pub struct LimineMemmapRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const LimineMemmapResponse,
}

#[repr(C)]
pub struct LimineHhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

#[repr(C)]
pub struct LimineHhdmRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const LimineHhdmResponse,
}

#[repr(C)]
pub struct LimineVideoMode {
    pub pitch: u64,
    pub width: u64,
    pub height: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

#[repr(C)]
pub struct LimineFramebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    pub unused: [u8; 7],
    pub edid_size: u64,
    pub edid: *const u8,
    pub mode_count: u64,
    pub modes: *const *const LimineVideoMode,
}

#[repr(C)]
pub struct LimineFramebufferResponse {
    pub revision: u64,
    pub framebuffer_count: u64,
    pub framebuffers: *const *const LimineFramebuffer,
}

#[repr(C)]
pub struct LimineFramebufferRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const LimineFramebufferResponse,
}

#[repr(C)]
pub struct LimineFile {
    pub revision: u64,
    pub address: *const u8,
    pub size: u64,
    pub path: *const i8,
    pub string: *const i8,
    pub media_type: u32,
    pub unused: u32,
    pub tftp_ip: u32,
    pub tftp_port: u32,
    pub partition_index: u32,
    pub mbr_disk_id: u32,
    pub gpt_disk_uuid: [u8; 16],
    pub gpt_part_uuid: [u8; 16],
    pub part_uuid: [u8; 16],
}

#[repr(C)]
pub struct LimineModuleResponse {
    pub revision: u64,
    pub module_count: u64,
    pub modules: *const *const LimineFile,
}

#[repr(C)]
pub struct LimineModuleRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const LimineModuleResponse,
    pub internal_module_count: u64,
    pub internal_modules: *const *const u8,
}

#[repr(C)]
pub struct LimineRsdpResponse {
    pub revision: u64,
    pub address: u64,
}

#[repr(C)]
pub struct LimineRsdpRequest {
    pub id: [u64; 4],
    pub revision: u64,
    pub response: *const LimineRsdpResponse,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MemoryMapEntryType {
    Usable,
    Reserved,
    AcpiReclaimable,
    AcpiNvs,
    BadMemory,
    BootloaderReclaimable,
    ExecutableAndModules,
    Framebuffer,
    Unknown(u64),
}

impl MemoryMapEntryType {
    fn from_raw(raw: u64) -> Self {
        match raw {
            0 => Self::Usable,
            1 => Self::Reserved,
            2 => Self::AcpiReclaimable,
            3 => Self::AcpiNvs,
            4 => Self::BadMemory,
            5 => Self::BootloaderReclaimable,
            6 => Self::ExecutableAndModules,
            7 => Self::Framebuffer,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Copy, Clone)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: MemoryMapEntryType,
}

#[derive(Copy, Clone)]
pub struct Framebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
}

#[derive(Copy, Clone)]
pub struct Module {
    file: *const LimineFile,
}

impl MemoryMapEntry {
    pub fn is_usable(self) -> bool {
        self.entry_type == MemoryMapEntryType::Usable
    }
}

impl Module {
    pub fn size(&self) -> usize {
        unsafe { (*self.file).size as usize }
    }

    pub fn bytes(&self) -> &'static [u8] {
        unsafe { core::slice::from_raw_parts((*self.file).address, self.size()) }
    }

    pub fn path(&self) -> Option<&'static str> {
        c_str(unsafe { (*self.file).path })
    }

    pub fn string(&self) -> Option<&'static str> {
        c_str(unsafe { (*self.file).string })
    }
}

pub struct MemoryMap {
    response: *const LimineMemmapResponse,
}

pub struct ModuleList {
    response: *const LimineModuleResponse,
}

pub struct MemoryMapIter {
    entries: *const *const LimineMemmapEntry,
    index: usize,
    len: usize,
}

pub struct ModuleIter {
    modules: *const *const LimineFile,
    index: usize,
    len: usize,
}

impl MemoryMap {
    pub fn len(&self) -> usize {
        unsafe { (*self.response).entry_count as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> MemoryMapIter {
        unsafe {
            MemoryMapIter {
                entries: (*self.response).entries,
                index: 0,
                len: (*self.response).entry_count as usize,
            }
        }
    }
}

impl ModuleList {
    pub fn len(&self) -> usize {
        unsafe { (*self.response).module_count as usize }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn iter(&self) -> ModuleIter {
        unsafe {
            ModuleIter {
                modules: (*self.response).modules,
                index: 0,
                len: (*self.response).module_count as usize,
            }
        }
    }
}

impl Iterator for MemoryMapIter {
    type Item = MemoryMapEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }

        unsafe {
            let slot = self.entries.add(self.index);
            self.index += 1;
            let entry_ptr = *slot;
            let entry = &*entry_ptr;
            Some(MemoryMapEntry {
                base: entry.base,
                length: entry.length,
                entry_type: MemoryMapEntryType::from_raw(entry.entry_type),
            })
        }
    }
}

impl Iterator for ModuleIter {
    type Item = Module;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.len {
            return None;
        }

        unsafe {
            let slot = self.modules.add(self.index);
            self.index += 1;
            let file = *slot;
            if file.is_null() {
                return None;
            }

            Some(Module { file })
        }
    }
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut LIMINE_BASE_REVISION: [u64; 3] = [
    0xf956_2b2d_5c95_a6c8,
    0x6a7b_3849_4453_6bdc,
    3,
];

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut MEMMAP_REQUEST: LimineMemmapRequest = LimineMemmapRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x67cf_3d9d_378a_806f,
        0xe304_acdf_c50c_3c62,
    ],
    revision: 0,
    response: core::ptr::null(),
};

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut HHDM_REQUEST: LimineHhdmRequest = LimineHhdmRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x48dc_f1cb_8ad2_b852,
        0x6398_4e95_9a98_244b,
    ],
    revision: 0,
    response: core::ptr::null(),
};

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut FRAMEBUFFER_REQUEST: LimineFramebufferRequest = LimineFramebufferRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x9d58_27dc_d881_dd75,
        0xa314_8604_f6fa_b11b,
    ],
    revision: 0,
    response: core::ptr::null(),
};

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut MODULE_REQUEST: LimineModuleRequest = LimineModuleRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0x3e7e_2797_02be_32af,
        0xca1c_4f3b_d128_0cee,
    ],
    revision: 0,
    response: core::ptr::null(),
    internal_module_count: 0,
    internal_modules: core::ptr::null(),
};

#[used]
#[unsafe(link_section = ".limine_requests")]
static mut RSDP_REQUEST: LimineRsdpRequest = LimineRsdpRequest {
    id: [
        0xc7b1_dd30_df4c_8b88,
        0x0a82_e883_a194_f07b,
        0xc5e7_7b6b_397e_7b43,
        0x2763_7845_accd_cf3c,
    ],
    revision: 0,
    response: core::ptr::null(),
};

#[used]
#[unsafe(link_section = ".limine_requests_start")]
static LIMINE_REQUESTS_START_MARKER: [u64; 4] = [
    0xf6b8_f4b3_9de7_d1ae,
    0xfab9_1a69_40fc_b9cf,
    0x785c_6ed0_15d3_e316,
    0x181e_920a_7852_b9d9,
];

#[used]
#[unsafe(link_section = ".limine_requests_end")]
static LIMINE_REQUESTS_END_MARKER: [u64; 2] = [
    0xadc0_e053_1bb1_0d03,
    0x9572_709f_3176_4c62,
];

pub fn base_revision_supported() -> bool {
    let revision = ptr::addr_of!(LIMINE_BASE_REVISION);
    unsafe { (*revision)[2] == 0 }
}

pub fn memory_map() -> Option<MemoryMap> {
    let request = ptr::addr_of!(MEMMAP_REQUEST);
    let response = unsafe { (*request).response };
    if response.is_null() {
        None
    } else {
        Some(MemoryMap { response })
    }
}

pub fn hhdm_offset() -> Option<u64> {
    let request = ptr::addr_of!(HHDM_REQUEST);
    let response = unsafe { (*request).response };
    if response.is_null() {
        None
    } else {
        Some(unsafe { (*response).offset })
    }
}

pub fn framebuffer() -> Option<Framebuffer> {
    let request = ptr::addr_of!(FRAMEBUFFER_REQUEST);
    let response = unsafe { (*request).response };
    if response.is_null() || unsafe { (*response).framebuffer_count } == 0 {
        return None;
    }

    unsafe {
        let framebuffer_ptr = *(*response).framebuffers;
        if framebuffer_ptr.is_null() {
            return None;
        }

        let framebuffer = &*framebuffer_ptr;
        Some(Framebuffer {
            address: framebuffer.address,
            width: framebuffer.width,
            height: framebuffer.height,
            pitch: framebuffer.pitch,
            bpp: framebuffer.bpp,
            memory_model: framebuffer.memory_model,
            red_mask_size: framebuffer.red_mask_size,
            red_mask_shift: framebuffer.red_mask_shift,
            green_mask_size: framebuffer.green_mask_size,
            green_mask_shift: framebuffer.green_mask_shift,
            blue_mask_size: framebuffer.blue_mask_size,
            blue_mask_shift: framebuffer.blue_mask_shift,
        })
    }
}

pub fn modules() -> Option<ModuleList> {
    let request = ptr::addr_of!(MODULE_REQUEST);
    let response = unsafe { (*request).response };
    if response.is_null() {
        None
    } else {
        Some(ModuleList { response })
    }
}

pub fn rsdp_address() -> Option<u64> {
    let request = ptr::addr_of!(RSDP_REQUEST);
    let response = unsafe { (*request).response };
    if response.is_null() {
        None
    } else {
        Some(unsafe { (*response).address })
    }
}

fn c_str(ptr: *const i8) -> Option<&'static str> {
    if ptr.is_null() {
        return None;
    }

    unsafe { CStr::from_ptr(ptr).to_str().ok() }
}
