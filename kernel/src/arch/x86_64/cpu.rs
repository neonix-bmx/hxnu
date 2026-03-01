use core::arch::x86_64::__cpuid;

const IA32_APIC_BASE_MSR: u32 = 0x1b;

#[derive(Copy, Clone)]
pub struct CpuInfo {
    pub local_apic_supported: bool,
    pub x2apic_supported: bool,
    pub tsc_deadline_supported: bool,
    pub apic_base: u64,
    pub apic_global_enabled: bool,
    pub x2apic_enabled: bool,
    pub bootstrap_processor: bool,
}

pub fn probe() -> CpuInfo {
    let leaf_1 = __cpuid(1);
    let local_apic_supported = (leaf_1.edx & (1 << 9)) != 0;
    let x2apic_supported = (leaf_1.ecx & (1 << 21)) != 0;
    let tsc_deadline_supported = (leaf_1.ecx & (1 << 24)) != 0;

    let apic_base_msr = if local_apic_supported {
        read_msr(IA32_APIC_BASE_MSR)
    } else {
        0
    };

    CpuInfo {
        local_apic_supported,
        x2apic_supported,
        tsc_deadline_supported,
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
