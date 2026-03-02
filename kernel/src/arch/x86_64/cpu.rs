const IA32_APIC_BASE_MSR: u32 = 0x1b;

use super::cpuid;

#[derive(Clone)]
pub struct CpuInfo {
    pub vendor: cpuid::CpuVendor,
    pub vendor_id: [u8; 12],
    pub brand: [u8; 48],
    pub max_basic_leaf: u32,
    pub max_extended_leaf: u32,
    pub hypervisor_present: bool,
    pub local_apic_supported: bool,
    pub x2apic_supported: bool,
    pub tsc_deadline_supported: bool,
    pub invariant_tsc_supported: bool,
    pub nx_supported: bool,
    pub initial_apic_id: u32,
    pub apic_base: u64,
    pub apic_global_enabled: bool,
    pub x2apic_enabled: bool,
    pub bootstrap_processor: bool,
}

impl CpuInfo {
    pub fn vendor_str(&self) -> &str {
        core::str::from_utf8(&self.vendor_id).unwrap_or("unknown")
    }

    pub fn brand_str(&self) -> Option<&str> {
        let end = self
            .brand
            .iter()
            .rposition(|byte| *byte != 0)
            .map(|index| index + 1)?;
        let text = core::str::from_utf8(&self.brand[..end]).ok()?.trim();
        if text.is_empty() { None } else { Some(text) }
    }
}

pub fn probe() -> CpuInfo {
    let identity = cpuid::read_identity();
    let leaf_1 = cpuid::query(1);
    let local_apic_supported = (leaf_1.edx & (1 << 9)) != 0;
    let x2apic_supported = (leaf_1.ecx & (1 << 21)) != 0;
    let tsc_deadline_supported = (leaf_1.ecx & (1 << 24)) != 0;
    let initial_apic_id = (leaf_1.ebx >> 24) & 0xff;
    let invariant_tsc_supported = if identity.max_extended_leaf >= 0x8000_0007 {
        (cpuid::query(0x8000_0007).edx & (1 << 8)) != 0
    } else {
        false
    };
    let nx_supported = if identity.max_extended_leaf >= 0x8000_0001 {
        (cpuid::query(0x8000_0001).edx & (1 << 20)) != 0
    } else {
        false
    };

    let apic_base_msr = if local_apic_supported {
        read_msr(IA32_APIC_BASE_MSR)
    } else {
        0
    };

    CpuInfo {
        vendor: identity.vendor,
        vendor_id: identity.vendor_id,
        brand: identity.brand,
        max_basic_leaf: identity.max_basic_leaf,
        max_extended_leaf: identity.max_extended_leaf,
        hypervisor_present: identity.hypervisor_present,
        local_apic_supported,
        x2apic_supported,
        tsc_deadline_supported,
        invariant_tsc_supported,
        nx_supported,
        initial_apic_id,
        apic_base: apic_base_msr & 0xffff_f000,
        apic_global_enabled: (apic_base_msr & (1 << 11)) != 0,
        x2apic_enabled: (apic_base_msr & (1 << 10)) != 0,
        bootstrap_processor: (apic_base_msr & (1 << 8)) != 0,
    }
}

fn read_msr(msr: u32) -> u64 {
    let low: u32;
    let high: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags),
        );
    }
    ((high as u64) << 32) | (low as u64)
}
