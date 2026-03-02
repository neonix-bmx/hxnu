use core::alloc::Layout;
use core::arch::asm;
use core::fmt;
use core::fmt::Write;
use core::panic::PanicInfo;

use crate::serial;
use crate::tty;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    serial::init();
    disable_interrupts();
    write_fatal_line(format_args!(
        "==================== KERNEL PANIC ===================="
    ));
    if let Some(location) = info.location() {
        write_fatal_line(format_args!(
            "location  {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        ));
    } else {
        write_fatal_line(format_args!("location  <unknown>"));
    }
    write_fatal_line(format_args!("message   {}", info.message()));
    write_fatal_line(format_args!("action    cpu halted"));
    write_fatal_line(format_args!("======================================================"));

    halt_forever()
}

#[alloc_error_handler]
fn alloc_error(layout: Layout) -> ! {
    serial::init();
    disable_interrupts();
    write_fatal_line(format_args!(
        "================= ALLOCATION FAILURE ================="
    ));
    write_fatal_line(format_args!("size      {}", layout.size()));
    write_fatal_line(format_args!("align     {}", layout.align()));
    write_fatal_line(format_args!("action    cpu halted"));
    write_fatal_line(format_args!("======================================================"));

    halt_forever()
}

pub(crate) fn write_fatal_line(args: fmt::Arguments<'_>) {
    let mut writer = FatalWriter;
    let _ = writer.write_fmt(args);
    let _ = writer.write_str("\n");
}

fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
}

fn halt_forever() -> ! {
    loop {
        unsafe {
            asm!("hlt", options(nomem, nostack));
        }
    }
}

struct FatalWriter;

impl fmt::Write for FatalWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        tty::write_style(tty::ConsoleStyle::Fatal, text);
        Ok(())
    }
}
