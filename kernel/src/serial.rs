use core::arch::asm;
use core::fmt;

const COM1_PORT: u16 = 0x3F8;
const DATA_REGISTER: u16 = COM1_PORT;
const INTERRUPT_ENABLE_REGISTER: u16 = COM1_PORT + 1;
const FIFO_CONTROL_REGISTER: u16 = COM1_PORT + 2;
const LINE_CONTROL_REGISTER: u16 = COM1_PORT + 3;
const MODEM_CONTROL_REGISTER: u16 = COM1_PORT + 4;
const LINE_STATUS_REGISTER: u16 = COM1_PORT + 5;
const TRANSMIT_HOLDING_REGISTER_EMPTY: u8 = 1 << 5;

pub fn init() {
    unsafe {
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x80);
        outb(DATA_REGISTER, 0x03);
        outb(INTERRUPT_ENABLE_REGISTER, 0x00);
        outb(LINE_CONTROL_REGISTER, 0x03);
        outb(FIFO_CONTROL_REGISTER, 0xC7);
        outb(MODEM_CONTROL_REGISTER, 0x0B);
    }
}

pub fn write_str(text: &str) {
    for byte in text.bytes() {
        if byte == b'\n' {
            write_byte(b'\r');
        }
        write_byte(byte);
    }
}

pub struct SerialWriter;

impl SerialWriter {
    pub const fn new() -> Self {
        Self
    }
}

impl fmt::Write for SerialWriter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        write_str(text);
        Ok(())
    }
}

fn write_byte(byte: u8) {
    unsafe {
        while (inb(LINE_STATUS_REGISTER) & TRANSMIT_HOLDING_REGISTER_EMPTY) == 0 {}
        outb(DATA_REGISTER, byte);
    }
}

unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") port, options(nomem, nostack, preserves_flags));
    }
    value
}
