use core::arch::asm;
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::ptr;

use crate::kprintln;

use super::apic;

#[repr(C)]
#[derive(Copy, Clone)]
struct InterruptStackFrame {
    instruction_pointer: u64,
    code_segment: u64,
    cpu_flags: u64,
    stack_pointer: u64,
    stack_segment: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
struct IdtEntry {
    pointer_low: u16,
    selector: u16,
    ist: u8,
    type_attributes: u8,
    pointer_middle: u16,
    pointer_high: u32,
    reserved: u32,
}

impl IdtEntry {
    const fn missing() -> Self {
        Self {
            pointer_low: 0,
            selector: 0,
            ist: 0,
            type_attributes: 0,
            pointer_middle: 0,
            pointer_high: 0,
            reserved: 0,
        }
    }

    fn set_handler_addr(&mut self, handler: usize, selector: u16) {
        self.pointer_low = handler as u16;
        self.selector = selector;
        self.ist = 0;
        self.type_attributes = 0x8e;
        self.pointer_middle = (handler >> 16) as u16;
        self.pointer_high = (handler >> 32) as u32;
        self.reserved = 0;
    }
}

#[repr(C, align(16))]
struct Idt {
    entries: [IdtEntry; 256],
}

impl Idt {
    const fn new() -> Self {
        Self {
            entries: [IdtEntry::missing(); 256],
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

struct GlobalIdt(UnsafeCell<Idt>);

unsafe impl Sync for GlobalIdt {}

impl GlobalIdt {
    const fn new() -> Self {
        Self(UnsafeCell::new(Idt::new()))
    }

    fn get(&self) -> *mut Idt {
        self.0.get()
    }
}

static IDT: GlobalIdt = GlobalIdt::new();

pub fn initialize() {
    let idt = unsafe { &mut *IDT.get() };
    let code_selector = read_code_segment();
    idt.entries[3].set_handler_addr(breakpoint_handler as *const () as usize, code_selector);
    idt.entries[6].set_handler_addr(invalid_opcode_handler as *const () as usize, code_selector);
    idt.entries[8].set_handler_addr(double_fault_handler as *const () as usize, code_selector);
    idt.entries[13].set_handler_addr(
        general_protection_fault_handler as *const () as usize,
        code_selector,
    );
    idt.entries[14].set_handler_addr(page_fault_handler as *const () as usize, code_selector);
    idt.entries[apic::TIMER_VECTOR].set_handler_addr(timer_handler as *const () as usize, code_selector);
    idt.entries[apic::SPURIOUS_VECTOR].set_handler_addr(
        spurious_interrupt_handler as *const () as usize,
        code_selector,
    );

    let idtr = DescriptorTablePointer {
        limit: (size_of::<Idt>() - 1) as u16,
        base: (idt as *const Idt) as u64,
    };

    unsafe {
        asm!("lidt [{idtr}]", idtr = in(reg) &idtr, options(readonly, nostack, preserves_flags));
    }
}

pub fn trigger_breakpoint() {
    unsafe {
        asm!("int3", options(nomem, nostack, preserves_flags));
    }
}

pub fn trigger_page_fault() {
    let fault_address = 0x0000_4000_0000_0000usize as *const u64;
    unsafe {
        ptr::read_volatile(fault_address);
    }
}

pub fn trigger_general_protection_fault() {
    unsafe {
        asm!(
            "mov rax, 0x23",
            "push rax",
            "lea rax, [rip + 2f]",
            "push rax",
            "retfq",
            "2:",
            lateout("rax") _,
            options(preserves_flags),
        );
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    kprintln!(
        "HXNU: exception breakpoint rip={:#018x} rsp={:#018x}",
        stack_frame.instruction_pointer,
        stack_frame.stack_pointer
    );
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    report_fatal_exception("invalid opcode", &stack_frame, None, None);
    halt_forever();
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, error_code: u64) -> ! {
    report_fatal_exception("double fault", &stack_frame, Some(error_code), None);
    halt_forever();
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    report_fatal_exception("general protection fault", &stack_frame, Some(error_code), None);
    halt_forever();
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    let fault_address: u64;
    unsafe {
        asm!("mov {}, cr2", out(reg) fault_address, options(nomem, nostack, preserves_flags));
    }

    report_fatal_exception("page fault", &stack_frame, Some(error_code), Some(fault_address));
    halt_forever();
}

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    let tick = apic::handle_timer_interrupt();
    crate::sched::on_timer_interrupt(tick);
}

extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    apic::handle_spurious_interrupt();
}

fn report_fatal_exception(
    kind: &str,
    stack_frame: &InterruptStackFrame,
    error_code: Option<u64>,
    fault_address: Option<u64>,
) {
    kprintln!("HXNU: ================= FATAL EXCEPTION ==================");
    kprintln!("HXNU: kind      {}", kind);
    kprintln!("HXNU: rip       {:#018x}", stack_frame.instruction_pointer);
    kprintln!("HXNU: cs        {:#06x}", stack_frame.code_segment);
    kprintln!("HXNU: rflags    {:#018x}", stack_frame.cpu_flags);
    kprintln!("HXNU: rsp       {:#018x}", stack_frame.stack_pointer);
    kprintln!("HXNU: ss        {:#06x}", stack_frame.stack_segment);
    if let Some(error_code) = error_code {
        kprintln!("HXNU: error     {:#x}", error_code);
    }
    if let Some(fault_address) = fault_address {
        kprintln!("HXNU: cr2       {:#018x}", fault_address);
        kprintln!(
            "HXNU: decode    present={} write={} user={} reserved={} instruction={}",
            yes_no(error_code.unwrap_or(0) & (1 << 0) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 1) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 2) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 3) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 4) != 0),
        );
    }
    kprintln!("HXNU: action    cpu halted");
    kprintln!("HXNU: =====================================================");
}

fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

fn read_code_segment() -> u16 {
    let code_segment: u16;
    unsafe {
        asm!("mov {segment:x}, cs", segment = out(reg) code_segment, options(nomem, nostack, preserves_flags));
    }
    code_segment
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
