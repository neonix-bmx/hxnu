use core::fmt;
use core::fmt::Write;
use core::arch::asm;

use crate::time;
use crate::tty;

#[allow(dead_code)]
pub fn write(args: fmt::Arguments<'_>) {
    let _interrupt_guard = InterruptGuard::new();
    let mut writer = KernelWriter {
        style: tty::ConsoleStyle::Default,
    };
    let _ = writer.write_fmt(args);
}

pub fn write_record(args: fmt::Arguments<'_>) {
    write_record_with_style(tty::ConsoleStyle::Default, args);
}

pub fn write_record_with_style(style: tty::ConsoleStyle, args: fmt::Arguments<'_>) {
    let _interrupt_guard = InterruptGuard::new();
    let mut writer = KernelWriter { style };
    let timestamp = time::timestamp();
    let _ = write!(writer, "[{}.{:09}] ", timestamp.seconds, timestamp.nanoseconds);
    let _ = writer.write_fmt(args);
    let _ = writer.write_str("\n");
}

struct KernelWriter {
    style: tty::ConsoleStyle,
}

impl fmt::Write for KernelWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        match self.style {
            tty::ConsoleStyle::Default => tty::write_str(text),
            style => tty::write_style(style, text),
        }
        Ok(())
    }
}

struct InterruptGuard {
    interrupt_flag_was_set: bool,
}

impl InterruptGuard {
    fn new() -> Self {
        let flags = read_rflags();
        unsafe {
            asm!("cli", options(nomem, nostack, preserves_flags));
        }
        Self {
            interrupt_flag_was_set: (flags & (1 << 9)) != 0,
        }
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.interrupt_flag_was_set {
            unsafe {
                asm!("sti", options(nomem, nostack, preserves_flags));
            }
        }
    }
}

fn read_rflags() -> u64 {
    let value: u64;
    unsafe {
        asm!("pushfq", "pop {}", out(reg) value, options(nomem, preserves_flags));
    }
    value
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ({
        $crate::log::write(core::format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! kprintln {
    () => ({
        $crate::log::write_record(core::format_args!(""));
    });
    ($($arg:tt)*) => ({
        $crate::log::write_record(core::format_args!($($arg)*));
    });
}

#[macro_export]
macro_rules! kprintln_style {
    ($style:expr, $($arg:tt)*) => ({
        $crate::log::write_record_with_style($style, core::format_args!($($arg)*));
    });
}
