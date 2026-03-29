use core::arch::{asm, global_asm};
use core::cell::UnsafeCell;
use core::mem::size_of;
use core::ptr;

use crate::kprintln;
use crate::panic::write_fatal_line;
use crate::syscall::{self, SyscallAbi, SyscallAction};

use super::apic;

const SYSCALL_VECTOR: usize = 0x80;
const INTERRUPT_GATE: u8 = 0x8e;
const USER_INTERRUPT_GATE: u8 = 0xee;

unsafe extern "C" {
    fn hxnu_x86_64_syscall_entry();
}

global_asm!(
    r#"
    .global hxnu_x86_64_syscall_entry
    .type hxnu_x86_64_syscall_entry,@function
hxnu_x86_64_syscall_entry:
    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, rsp
    sub rsp, 8
    call hxnu_x86_64_handle_syscall_frame
    add rsp, 8

    mov [rsp], rax

    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15
    iretq
"#
);

#[repr(C)]
struct SyscallRegisterFrame {
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
}

#[derive(Copy, Clone)]
pub struct SyscallSelfTest {
    pub linux_write_result: i64,
    pub linux_openat_result: i64,
    pub linux_read_result: i64,
    pub linux_close_result: i64,
    pub linux_getpid_result: i64,
    pub ghost_open_result: i64,
    pub ghost_read_result: i64,
    pub ghost_close_result: i64,
    pub ghost_gettid_result: i64,
    pub hxnu_open_result: i64,
    pub hxnu_read_result: i64,
    pub hxnu_close_result: i64,
    pub hxnu_abi_version_result: i64,
}

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
        self.set_handler_addr_with_attributes(handler, selector, INTERRUPT_GATE);
    }

    fn set_handler_addr_with_attributes(
        &mut self,
        handler: usize,
        selector: u16,
        type_attributes: u8,
    ) {
        self.pointer_low = handler as u16;
        self.selector = selector;
        self.ist = 0;
        self.type_attributes = type_attributes;
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
    idt.entries[SYSCALL_VECTOR].set_handler_addr_with_attributes(
        hxnu_x86_64_syscall_entry as *const () as usize,
        code_selector,
        USER_INTERRUPT_GATE,
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

#[unsafe(no_mangle)]
extern "C" fn hxnu_x86_64_handle_syscall_frame(frame: &mut SyscallRegisterFrame) -> u64 {
    let abi = match decode_syscall_abi(frame.r12) {
        Some(abi) => abi,
        None => return (-38i64) as u64,
    };
    let outcome = syscall::dispatch(
        abi,
        frame.rax,
        [frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9],
    );
    match outcome.action {
        SyscallAction::Continue => outcome.value as u64,
        SyscallAction::ExitGroup { status } => {
            if let Some(record) = crate::sched::request_exit_group(status) {
                kprintln!(
                    "HXNU: syscall exit_group abi={} status={} exited={}#{} next={}#{} runqueue={}",
                    abi.as_str(),
                    record.status,
                    record.exited_thread_name,
                    record.exited_thread_id,
                    record.next_thread_name,
                    record.next_thread_id,
                    record.runqueue_depth,
                );
            } else {
                kprintln!(
                    "HXNU: syscall exit_group abi={} status={} scheduler=unavailable",
                    abi.as_str(),
                    status
                );
            }
            outcome.value as u64
        }
    }
}

pub fn run_syscall_self_test() -> SyscallSelfTest {
    static LINUX_SMOKE: &[u8] = b"HXNU: int 0x80 linux syscall self-test\n";
    static OPEN_PATH: &[u8] = b"/proc/version\0";

    let linux_write_result = invoke_syscall(
        SyscallAbi::LinuxBootstrap,
        syscall::LINUX_SYS_WRITE,
        [
            1,
            LINUX_SMOKE.as_ptr() as u64,
            LINUX_SMOKE.len() as u64,
            0,
            0,
            0,
        ],
    );
    let linux_openat_result = invoke_syscall(
        SyscallAbi::LinuxBootstrap,
        syscall::LINUX_SYS_OPENAT,
        [(-100i64) as u64, OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0],
    );
    let mut linux_read_buffer = [0u8; 48];
    let mut linux_read_result = -9;
    let mut linux_close_result = -9;
    if linux_openat_result >= 0 {
        let fd = linux_openat_result as u64;
        linux_read_result = invoke_syscall(
            SyscallAbi::LinuxBootstrap,
            syscall::LINUX_SYS_READ,
            [
                fd,
                linux_read_buffer.as_mut_ptr() as u64,
                linux_read_buffer.len() as u64,
                0,
                0,
                0,
            ],
        );
        linux_close_result =
            invoke_syscall(SyscallAbi::LinuxBootstrap, syscall::LINUX_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]);
    }

    let linux_getpid_result = invoke_syscall(SyscallAbi::LinuxBootstrap, syscall::LINUX_SYS_GETPID, [0; 6]);

    let ghost_open_result = invoke_syscall(
        SyscallAbi::GhostBootstrap,
        syscall::GHOST_SYS_OPEN,
        [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0],
    );
    let mut ghost_read_buffer = [0u8; 48];
    let mut ghost_read_result = -9;
    let mut ghost_close_result = -9;
    if ghost_open_result >= 0 {
        let fd = ghost_open_result as u64;
        ghost_read_result = invoke_syscall(
            SyscallAbi::GhostBootstrap,
            syscall::GHOST_SYS_READ,
            [
                fd,
                ghost_read_buffer.as_mut_ptr() as u64,
                ghost_read_buffer.len() as u64,
                0,
                0,
                0,
            ],
        );
        ghost_close_result =
            invoke_syscall(SyscallAbi::GhostBootstrap, syscall::GHOST_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]);
    }
    let ghost_gettid_result = invoke_syscall(SyscallAbi::GhostBootstrap, syscall::GHOST_SYS_GETTID, [0; 6]);

    let hxnu_open_result = invoke_syscall(
        SyscallAbi::HxnuNativeBootstrap,
        syscall::HXNU_SYS_OPEN,
        [OPEN_PATH.as_ptr() as u64, 0, 0, 0, 0, 0],
    );
    let mut hxnu_read_buffer = [0u8; 48];
    let mut hxnu_read_result = -9;
    let mut hxnu_close_result = -9;
    if hxnu_open_result >= 0 {
        let fd = hxnu_open_result as u64;
        hxnu_read_result = invoke_syscall(
            SyscallAbi::HxnuNativeBootstrap,
            syscall::HXNU_SYS_READ,
            [
                fd,
                hxnu_read_buffer.as_mut_ptr() as u64,
                hxnu_read_buffer.len() as u64,
                0,
                0,
                0,
            ],
        );
        hxnu_close_result =
            invoke_syscall(SyscallAbi::HxnuNativeBootstrap, syscall::HXNU_SYS_CLOSE, [fd, 0, 0, 0, 0, 0]);
    }
    let hxnu_abi_version_result = invoke_syscall(
        SyscallAbi::HxnuNativeBootstrap,
        syscall::HXNU_SYS_ABI_VERSION,
        [0; 6],
    );

    SyscallSelfTest {
        linux_write_result,
        linux_openat_result,
        linux_read_result,
        linux_close_result,
        linux_getpid_result,
        ghost_open_result,
        ghost_read_result,
        ghost_close_result,
        ghost_gettid_result,
        hxnu_open_result,
        hxnu_read_result,
        hxnu_close_result,
        hxnu_abi_version_result,
    }
}

fn invoke_syscall(abi: SyscallAbi, number: u64, args: [u64; 6]) -> i64 {
    let mut result = number;
    unsafe {
        asm!(
            "int 0x80",
            inlateout("rax") result,
            in("r12") syscall_abi_selector(abi),
            in("rdi") args[0],
            in("rsi") args[1],
            in("rdx") args[2],
            in("r10") args[3],
            in("r8") args[4],
            in("r9") args[5],
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    result as i64
}

const fn syscall_abi_selector(abi: SyscallAbi) -> u64 {
    match abi {
        SyscallAbi::LinuxBootstrap => 0,
        SyscallAbi::GhostBootstrap => 1,
        SyscallAbi::HxnuNativeBootstrap => 2,
    }
}

const fn decode_syscall_abi(selector: u64) -> Option<SyscallAbi> {
    match selector {
        0 => Some(SyscallAbi::LinuxBootstrap),
        1 => Some(SyscallAbi::GhostBootstrap),
        2 => Some(SyscallAbi::HxnuNativeBootstrap),
        _ => None,
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
    write_fatal_line(format_args!(
        "================= FATAL EXCEPTION =================="
    ));
    write_fatal_line(format_args!("kind      {}", kind));
    write_fatal_line(format_args!("rip       {:#018x}", stack_frame.instruction_pointer));
    write_fatal_line(format_args!("cs        {:#06x}", stack_frame.code_segment));
    write_fatal_line(format_args!("rflags    {:#018x}", stack_frame.cpu_flags));
    write_fatal_line(format_args!("rsp       {:#018x}", stack_frame.stack_pointer));
    write_fatal_line(format_args!("ss        {:#06x}", stack_frame.stack_segment));
    if let Some(error_code) = error_code {
        write_fatal_line(format_args!("error     {:#x}", error_code));
    }
    if let Some(fault_address) = fault_address {
        write_fatal_line(format_args!("cr2       {:#018x}", fault_address));
        write_fatal_line(format_args!(
            "decode    present={} write={} user={} reserved={} instruction={}",
            yes_no(error_code.unwrap_or(0) & (1 << 0) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 1) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 2) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 3) != 0),
            yes_no(error_code.unwrap_or(0) & (1 << 4) != 0),
        ));
    }
    write_fatal_line(format_args!("action    cpu halted"));
    write_fatal_line(format_args!("===================================================="));
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
