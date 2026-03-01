use core::arch::asm;
use core::mem::size_of;

const KERNEL_CODE_SELECTOR: u16 = 0x28;
const KERNEL_DATA_SELECTOR: u16 = 0x30;

const GDT_ENTRIES: [u64; 7] = [
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    0x0000_0000_0000_0000,
    // Pre-set the accessed bits so segment loads do not try to mutate read-only mappings.
    0x00af_9b00_0000_ffff,
    0x00cf_9300_0000_ffff,
];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

#[derive(Copy, Clone)]
pub struct SegmentSelectors {
    pub cs: u16,
    pub ds: u16,
    pub es: u16,
    pub fs: u16,
    pub gs: u16,
    pub ss: u16,
}

pub fn read_segment_selectors() -> SegmentSelectors {
    let cs = read_cs();
    let ds = read_ds();
    let es = read_es();
    let fs = read_fs();
    let gs = read_gs();
    let ss = read_ss();

    SegmentSelectors { cs, ds, es, fs, gs, ss }
}

pub fn load_table_only() {
    let gdtr = DescriptorTablePointer {
        limit: (size_of::<[u64; 7]>() - 1) as u16,
        base: (&GDT_ENTRIES as *const [u64; 7]) as u64,
    };

    unsafe {
        asm!(
            "lgdt [{gdtr}]",
            gdtr = in(reg) &gdtr,
            options(readonly, nostack, preserves_flags),
        );
    }
}

pub fn reload_code_segment() {
    unsafe {
        asm!(
            "mov rdx, {code_selector}",
            "push rdx",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            code_selector = in(reg) (KERNEL_CODE_SELECTOR as u64),
            lateout("rax") _,
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_data_segments() {
    reload_ds();
    reload_es();
    reload_fs();
    reload_gs();
    reload_ss();
}

pub fn reload_ds() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov ds, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_es() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov es, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_fs() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov fs, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_gs() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov gs, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn reload_ss() {
    unsafe {
        asm!(
            "mov rdx, {data_selector}",
            "mov ax, dx",
            "mov ss, ax",
            data_selector = in(reg) (KERNEL_DATA_SELECTOR as u64),
            lateout("rdx") _,
            options(preserves_flags),
        );
    }
}

pub fn initialize() {
    load_table_only();
    reload_code_segment();
    reload_data_segments();
}

fn read_cs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, cs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_ds() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, ds", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_es() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, es", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_fs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, fs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_gs() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, gs", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn read_ss() -> u16 {
    let value: u16;
    unsafe {
        asm!("mov {segment:x}, ss", segment = out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}
