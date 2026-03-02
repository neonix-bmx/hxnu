use core::cell::UnsafeCell;

use crate::fb;
use crate::serial;

pub use crate::fb::ConsoleStyle;

const OUTPUT_SERIAL: u8 = 1 << 0;
const OUTPUT_FRAMEBUFFER: u8 = 1 << 1;

#[derive(Copy, Clone)]
pub struct TtyBootstrap {
    pub console_id: u32,
    pub output_count: u8,
    pub framebuffer_output: bool,
}

#[derive(Copy, Clone)]
pub struct TtyStats {
    pub console_id: u32,
    pub output_count: u8,
    pub bytes_written: u64,
    pub lines_written: u64,
}

struct GlobalTtyConsole(UnsafeCell<TtyConsole>);

unsafe impl Sync for GlobalTtyConsole {}

impl GlobalTtyConsole {
    const fn new() -> Self {
        Self(UnsafeCell::new(TtyConsole::new()))
    }

    fn get(&self) -> *mut TtyConsole {
        self.0.get()
    }
}

static TTY_CONSOLE: GlobalTtyConsole = GlobalTtyConsole::new();

struct TtyConsole {
    initialized: bool,
    console_id: u32,
    outputs: u8,
    bytes_written: u64,
    lines_written: u64,
}

impl TtyConsole {
    const fn new() -> Self {
        Self {
            initialized: false,
            console_id: 0,
            outputs: OUTPUT_SERIAL,
            bytes_written: 0,
            lines_written: 0,
        }
    }

    fn initialize(&mut self, framebuffer_output: bool) -> TtyBootstrap {
        self.initialized = true;
        self.console_id = 0;
        self.outputs = OUTPUT_SERIAL;
        if framebuffer_output {
            self.outputs |= OUTPUT_FRAMEBUFFER;
        }

        TtyBootstrap {
            console_id: self.console_id,
            output_count: output_count(self.outputs),
            framebuffer_output,
        }
    }

    fn write_str(&mut self, text: &str) {
        self.write_style(ConsoleStyle::Default, text);
    }

    fn write_style(&mut self, style: ConsoleStyle, text: &str) {
        let outputs = if self.initialized { self.outputs } else { OUTPUT_SERIAL };

        if outputs & OUTPUT_SERIAL != 0 {
            serial::write_str(text);
        }
        if outputs & OUTPUT_FRAMEBUFFER != 0 {
            match style {
                ConsoleStyle::Default => fb::write_str(text),
                style => fb::write_style(style, text),
            }
        }

        self.bytes_written = self.bytes_written.saturating_add(text.len() as u64);
        self.lines_written = self
            .lines_written
            .saturating_add(text.bytes().filter(|byte| *byte == b'\n').count() as u64);
    }

    fn stats(&self) -> TtyStats {
        let outputs = if self.initialized { self.outputs } else { OUTPUT_SERIAL };
        TtyStats {
            console_id: self.console_id,
            output_count: output_count(outputs),
            bytes_written: self.bytes_written,
            lines_written: self.lines_written,
        }
    }
}

pub fn initialize(framebuffer_output: bool) -> TtyBootstrap {
    unsafe { (*TTY_CONSOLE.get()).initialize(framebuffer_output) }
}

pub fn write_str(text: &str) {
    unsafe {
        (*TTY_CONSOLE.get()).write_str(text);
    }
}

pub fn write_style(style: ConsoleStyle, text: &str) {
    unsafe {
        (*TTY_CONSOLE.get()).write_style(style, text);
    }
}

pub fn stats() -> TtyStats {
    unsafe { (*TTY_CONSOLE.get()).stats() }
}

const fn output_count(outputs: u8) -> u8 {
    (outputs & OUTPUT_SERIAL != 0) as u8 + (outputs & OUTPUT_FRAMEBUFFER != 0) as u8
}
