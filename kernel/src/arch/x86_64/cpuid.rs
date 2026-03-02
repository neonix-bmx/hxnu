use core::arch::x86_64::{__cpuid, __cpuid_count, CpuidResult};
const VENDOR_INTEL: &[u8; 12] = b"GenuineIntel";
const VENDOR_AMD: &[u8; 12] = b"AuthenticAMD";

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum CpuVendor {
    Intel,
    Amd,
    Unknown,
}

impl CpuVendor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Intel => "intel",
            Self::Amd => "amd",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone)]
pub struct CpuIdentity {
    pub vendor: CpuVendor,
    pub vendor_id: [u8; 12],
    pub brand: [u8; 48],
    pub max_basic_leaf: u32,
    pub max_extended_leaf: u32,
    pub hypervisor_present: bool,
}

pub fn query(leaf: u32) -> CpuidResult {
    __cpuid(leaf)
}

pub fn query_count(leaf: u32, subleaf: u32) -> CpuidResult {
    __cpuid_count(leaf, subleaf)
}

pub fn max_basic_leaf() -> u32 {
    query(0).eax
}

pub fn max_extended_leaf() -> u32 {
    query(0x8000_0000).eax
}

pub fn read_identity() -> CpuIdentity {
    let vendor_leaf = query(0);
    let max_basic_leaf = vendor_leaf.eax;
    let mut vendor_id = [0u8; 12];
    vendor_id[0..4].copy_from_slice(&vendor_leaf.ebx.to_le_bytes());
    vendor_id[4..8].copy_from_slice(&vendor_leaf.edx.to_le_bytes());
    vendor_id[8..12].copy_from_slice(&vendor_leaf.ecx.to_le_bytes());

    let vendor = match &vendor_id {
        VENDOR_INTEL => CpuVendor::Intel,
        VENDOR_AMD => CpuVendor::Amd,
        _ => CpuVendor::Unknown,
    };

    let max_extended_leaf = max_extended_leaf();
    let leaf_1 = query(1);
    let hypervisor_present = (leaf_1.ecx & (1 << 31)) != 0;

    let mut brand = [0u8; 48];
    if max_extended_leaf >= 0x8000_0004 {
        for (index, leaf) in [0x8000_0002, 0x8000_0003, 0x8000_0004]
            .iter()
            .copied()
            .enumerate()
        {
            let result = query(leaf);
            let offset = index * 16;
            brand[offset..offset + 4].copy_from_slice(&result.eax.to_le_bytes());
            brand[offset + 4..offset + 8].copy_from_slice(&result.ebx.to_le_bytes());
            brand[offset + 8..offset + 12].copy_from_slice(&result.ecx.to_le_bytes());
            brand[offset + 12..offset + 16].copy_from_slice(&result.edx.to_le_bytes());
        }
    }

    CpuIdentity {
        vendor,
        vendor_id,
        brand,
        max_basic_leaf,
        max_extended_leaf,
        hypervisor_present,
    }
}
