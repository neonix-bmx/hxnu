use core::arch::global_asm;

const STACK_ALIGNMENT: usize = 16;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TaskContext {
    pub rsp: u64,
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl TaskContext {
    pub const fn empty() -> Self {
        Self {
            rsp: 0,
            rbx: 0,
            rbp: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}

pub fn initialize_kernel_thread(
    context: &mut TaskContext,
    stack: &'static mut [u8],
    entry: extern "C" fn() -> !,
) {
    let stack_base = stack.as_mut_ptr() as usize;
    let stack_end = stack_base + stack.len();
    let aligned_end = stack_end & !(STACK_ALIGNMENT - 1);
    let stack_words = aligned_end as *mut usize;

    unsafe {
        let thread_exit_slot = stack_words.sub(1);
        thread_exit_slot.write(thread_exit_trap as *const () as usize);
        let entry_slot = thread_exit_slot.sub(1);
        entry_slot.write(entry as usize);

        *context = TaskContext {
            rsp: entry_slot as u64,
            ..TaskContext::empty()
        };
    }
}

pub unsafe fn switch(current: &mut TaskContext, next: &TaskContext) -> ! {
    unsafe {
        hxnu_context_switch(current as *mut TaskContext, next as *const TaskContext);
    }
}

extern "C" fn thread_exit_trap() -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli", "hlt", options(nomem, nostack));
        }
    }
}

unsafe extern "C" {
    fn hxnu_context_switch(current: *mut TaskContext, next: *const TaskContext) -> !;
}

global_asm!(
    r#"
    .global hxnu_context_switch
    .type hxnu_context_switch,@function
hxnu_context_switch:
    mov [rdi + 0x00], rsp
    mov [rdi + 0x08], rbx
    mov [rdi + 0x10], rbp
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], r13
    mov [rdi + 0x28], r14
    mov [rdi + 0x30], r15

    mov rsp, [rsi + 0x00]
    mov rbx, [rsi + 0x08]
    mov rbp, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov r13, [rsi + 0x20]
    mov r14, [rsi + 0x28]
    mov r15, [rsi + 0x30]
    ret
"#
);
