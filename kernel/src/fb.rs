use core::cell::UnsafeCell;
use core::ptr::{read_volatile, write_volatile};

use crate::limine;

const LIMINE_FRAMEBUFFER_RGB: u8 = 1;

const HEADER_HEIGHT: u64 = 72;
const HEADER_ACCENT_Y: u64 = 72;
const HEADER_ACCENT_HEIGHT: u64 = 4;
const BANNER_X: u64 = 32;
const BANNER_Y: u64 = 112;
const BANNER_WIDTH: u64 = 256;
const BANNER_HEIGHT: u64 = 144;
const LOG_ORIGIN_X: u64 = 32;
const LOG_ORIGIN_Y: u64 = 288;
const LOG_BOTTOM_MARGIN: u64 = 32;

const FONT_SCALE: u64 = 2;
const GLYPH_WIDTH: u64 = 5;
const GLYPH_ADVANCE_X: u64 = 6;
const GLYPH_ADVANCE_Y: u64 = 8;
const CHAR_WIDTH: u64 = GLYPH_ADVANCE_X * FONT_SCALE;
const CHAR_HEIGHT: u64 = GLYPH_ADVANCE_Y * FONT_SCALE;

#[derive(Copy, Clone)]
pub struct FramebufferSummary {
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub sample_background: u32,
    pub sample_accent: u32,
}

#[derive(Copy, Clone)]
pub enum FramebufferError {
    UnsupportedMemoryModel,
    UnsupportedBitsPerPixel,
    MissingAddress,
    ConsoleAreaTooSmall,
}

impl FramebufferError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedMemoryModel => "unsupported framebuffer memory model",
            Self::UnsupportedBitsPerPixel => "unsupported framebuffer bits-per-pixel",
            Self::MissingAddress => "framebuffer address is null",
            Self::ConsoleAreaTooSmall => "framebuffer console area is too small",
        }
    }
}

#[derive(Copy, Clone)]
pub enum ConsoleStyle {
    Default,
    Accent,
    Success,
    Warning,
    Error,
    Fatal,
    Muted,
}

struct GlobalFramebufferConsole(UnsafeCell<Option<FramebufferConsole>>);

unsafe impl Sync for GlobalFramebufferConsole {}

impl GlobalFramebufferConsole {
    const fn new() -> Self {
        Self(UnsafeCell::new(None))
    }

    fn get(&self) -> *mut Option<FramebufferConsole> {
        self.0.get()
    }
}

static FRAMEBUFFER_CONSOLE: GlobalFramebufferConsole = GlobalFramebufferConsole::new();

pub fn initialize(framebuffer: limine::Framebuffer) -> Result<FramebufferSummary, FramebufferError> {
    if framebuffer.address.is_null() {
        return Err(FramebufferError::MissingAddress);
    }
    if framebuffer.memory_model != LIMINE_FRAMEBUFFER_RGB {
        return Err(FramebufferError::UnsupportedMemoryModel);
    }

    let bytes_per_pixel = framebuffer.bpp.div_ceil(8) as usize;
    if bytes_per_pixel < 3 || bytes_per_pixel > 4 {
        return Err(FramebufferError::UnsupportedBitsPerPixel);
    }

    let mut console = FramebufferConsole::new(framebuffer, bytes_per_pixel)?;
    console.paint_boot_surface();
    let sample_background = console.read_pixel(8, 8);
    let sample_accent = console.read_pixel(56, 136);

    unsafe {
        *FRAMEBUFFER_CONSOLE.get() = Some(console);
    }

    Ok(FramebufferSummary {
        width: framebuffer.width,
        height: framebuffer.height,
        pitch: framebuffer.pitch,
        bpp: framebuffer.bpp,
        sample_background,
        sample_accent,
    })
}

pub fn write_str(text: &str) {
    write_style(ConsoleStyle::Default, text);
}

pub fn write_style(style: ConsoleStyle, text: &str) {
    unsafe {
        if let Some(console) = &mut *FRAMEBUFFER_CONSOLE.get() {
            console.write_str(style, text);
        }
    }
}

pub fn console_probe() -> Option<u32> {
    unsafe { (&*FRAMEBUFFER_CONSOLE.get()).as_ref().map(FramebufferConsole::first_ink_sample) }
}

struct FramebufferConsole {
    framebuffer: limine::Framebuffer,
    bytes_per_pixel: usize,
    background: u32,
    header: u32,
    accent: u32,
    accent_soft: u32,
    text: u32,
    success: u32,
    warning: u32,
    error: u32,
    fatal: u32,
    muted: u32,
    log_origin_x: u64,
    log_origin_y: u64,
    log_width: u64,
    log_height: u64,
    columns: u64,
    rows: u64,
    cursor_column: u64,
    cursor_row: u64,
}

impl FramebufferConsole {
    fn new(framebuffer: limine::Framebuffer, bytes_per_pixel: usize) -> Result<Self, FramebufferError> {
        let mut console = Self {
            framebuffer,
            bytes_per_pixel,
            background: 0,
            header: 0,
            accent: 0,
            accent_soft: 0,
            text: 0,
            success: 0,
            warning: 0,
            error: 0,
            fatal: 0,
            muted: 0,
            log_origin_x: LOG_ORIGIN_X,
            log_origin_y: LOG_ORIGIN_Y,
            log_width: framebuffer.width.saturating_sub(LOG_ORIGIN_X * 2),
            log_height: framebuffer
                .height
                .saturating_sub(LOG_ORIGIN_Y)
                .saturating_sub(LOG_BOTTOM_MARGIN),
            columns: 0,
            rows: 0,
            cursor_column: 0,
            cursor_row: 0,
        };

        if console.log_width < CHAR_WIDTH * 4 || console.log_height < CHAR_HEIGHT * 4 {
            return Err(FramebufferError::ConsoleAreaTooSmall);
        }

        console.columns = console.log_width / CHAR_WIDTH;
        console.rows = console.log_height / CHAR_HEIGHT;
        console.log_width = console.columns * CHAR_WIDTH;
        console.log_height = console.rows * CHAR_HEIGHT;

        console.background = console.pack_rgb(0x09, 0x11, 0x1b);
        console.header = console.pack_rgb(0x11, 0x2b, 0x44);
        console.accent = console.pack_rgb(0x2d, 0xd4, 0xbf);
        console.accent_soft = console.pack_rgb(0x79, 0xe2, 0xd0);
        console.text = console.pack_rgb(0xf5, 0xf7, 0xfa);
        console.success = console.pack_rgb(0x8b, 0xe9, 0x7d);
        console.warning = console.pack_rgb(0xff, 0xc8, 0x57);
        console.error = console.pack_rgb(0xff, 0x6b, 0x6b);
        console.fatal = console.pack_rgb(0xff, 0x4d, 0x6d);
        console.muted = console.pack_rgb(0x94, 0xa3, 0xb8);

        Ok(console)
    }

    fn paint_boot_surface(&mut self) {
        self.clear(self.background);
        self.fill_rect(0, 0, self.framebuffer.width, HEADER_HEIGHT, self.header);
        self.fill_rect(0, HEADER_ACCENT_Y, self.framebuffer.width, HEADER_ACCENT_HEIGHT, self.accent);
        self.fill_rect(BANNER_X, BANNER_Y, BANNER_WIDTH, BANNER_HEIGHT, self.header);
        self.fill_rect(BANNER_X + 16, BANNER_Y + 16, 32, 96, self.accent);
        self.fill_rect(BANNER_X + 64, BANNER_Y + 16, 32, 96, self.accent_soft);
        self.fill_rect(BANNER_X + 112, BANNER_Y + 16, 32, 96, self.accent);
        self.fill_rect(BANNER_X + 160, BANNER_Y + 16, 32, 96, self.accent_soft);
        self.fill_rect(BANNER_X + 208, BANNER_Y + 16, 32, 96, self.accent);
        self.draw_text(BANNER_X, 20, "HXNU 2605", self.text, 3);
        self.draw_text(320, 32, "FB READY", self.text, 2);

        let mut mode_line = [0u8; 32];
        let mode_len = format_mode_line(
            &mut mode_line,
            self.framebuffer.width,
            self.framebuffer.height,
            self.framebuffer.bpp,
        );
        self.draw_text_bytes(320, 72, &mode_line[..mode_len], self.accent, 2);
        self.clear_log_region();
    }

    fn write_str(&mut self, style: ConsoleStyle, text: &str) {
        for byte in text.bytes() {
            match byte {
                b'\r' => {}
                b'\n' => self.new_line(),
                b'\t' => {
                    for _ in 0..4 {
                        self.write_byte(style, b' ');
                    }
                }
                byte => self.write_byte(style, normalize_glyph_byte(byte)),
            }
        }
    }

    fn write_byte(&mut self, style: ConsoleStyle, byte: u8) {
        if self.cursor_column >= self.columns {
            self.new_line();
        }

        let x = self.log_origin_x + self.cursor_column * CHAR_WIDTH;
        let y = self.log_origin_y + self.cursor_row * CHAR_HEIGHT;
        self.fill_rect(x, y, CHAR_WIDTH, CHAR_HEIGHT, self.background);
        self.draw_glyph(
            x + FONT_SCALE,
            y + FONT_SCALE,
            byte,
            self.style_color(style),
            FONT_SCALE,
        );
        self.cursor_column += 1;
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row + 1 >= self.rows {
            self.scroll_up();
        } else {
            self.cursor_row += 1;
        }
    }

    fn scroll_up(&mut self) {
        let destination_y = self.log_origin_y;
        let end_y = self.log_origin_y + self.log_height - CHAR_HEIGHT;
        let end_x = self.log_origin_x + self.log_width;

        for y in destination_y..end_y {
            for x in self.log_origin_x..end_x {
                let color = self.read_pixel(x, y + CHAR_HEIGHT);
                self.write_pixel(x, y, color);
            }
        }

        self.fill_rect(
            self.log_origin_x,
            self.log_origin_y + self.log_height - CHAR_HEIGHT,
            self.log_width,
            CHAR_HEIGHT,
            self.background,
        );
        self.cursor_row = self.rows - 1;
    }

    fn clear_log_region(&mut self) {
        self.fill_rect(
            self.log_origin_x,
            self.log_origin_y,
            self.log_width,
            self.log_height,
            self.background,
        );
        self.cursor_column = 0;
        self.cursor_row = 0;
    }

    fn first_ink_sample(&self) -> u32 {
        let end_y = self.log_origin_y + (CHAR_HEIGHT * 2).min(self.log_height);
        let end_x = self.log_origin_x + (CHAR_WIDTH * 32).min(self.log_width);
        for y in self.log_origin_y..end_y {
            for x in self.log_origin_x..end_x {
                let color = self.read_pixel(x, y);
                if color != self.background {
                    return color;
                }
            }
        }
        self.background
    }

    fn style_color(&self, style: ConsoleStyle) -> u32 {
        match style {
            ConsoleStyle::Default => self.text,
            ConsoleStyle::Accent => self.accent,
            ConsoleStyle::Success => self.success,
            ConsoleStyle::Warning => self.warning,
            ConsoleStyle::Error => self.error,
            ConsoleStyle::Fatal => self.fatal,
            ConsoleStyle::Muted => self.muted,
        }
    }

    fn clear(&mut self, color: u32) {
        self.fill_rect(0, 0, self.framebuffer.width, self.framebuffer.height, color);
    }

    fn fill_rect(&mut self, x: u64, y: u64, width: u64, height: u64, color: u32) {
        let max_x = x.saturating_add(width).min(self.framebuffer.width);
        let max_y = y.saturating_add(height).min(self.framebuffer.height);

        for current_y in y..max_y {
            for current_x in x..max_x {
                self.write_pixel(current_x, current_y, color);
            }
        }
    }

    fn draw_text(&mut self, x: u64, y: u64, text: &str, color: u32, scale: u64) {
        self.draw_text_bytes(x, y, text.as_bytes(), color, scale);
    }

    fn draw_text_bytes(&mut self, x: u64, y: u64, text: &[u8], color: u32, scale: u64) {
        let mut cursor_x = x;
        for byte in text {
            self.draw_glyph(cursor_x, y, normalize_glyph_byte(*byte), color, scale);
            cursor_x = cursor_x.saturating_add(GLYPH_ADVANCE_X * scale);
        }
    }

    fn draw_glyph(&mut self, x: u64, y: u64, byte: u8, color: u32, scale: u64) {
        let glyph = glyph(byte);
        for (row_index, row_bits) in glyph.iter().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if row_bits & (1 << (GLYPH_WIDTH - 1 - column)) == 0 {
                    continue;
                }
                let pixel_x = x.saturating_add(column * scale);
                let pixel_y = y.saturating_add((row_index as u64) * scale);
                self.fill_rect(pixel_x, pixel_y, scale, scale, color);
            }
        }
    }

    fn write_pixel(&mut self, x: u64, y: u64, color: u32) {
        if x >= self.framebuffer.width || y >= self.framebuffer.height {
            return;
        }

        let offset = y
            .saturating_mul(self.framebuffer.pitch)
            .saturating_add(x.saturating_mul(self.bytes_per_pixel as u64)) as usize;

        unsafe {
            let base = self.framebuffer.address.add(offset);
            for byte_index in 0..self.bytes_per_pixel {
                let byte = ((color >> (byte_index * 8)) & 0xff) as u8;
                write_volatile(base.add(byte_index), byte);
            }
        }
    }

    fn read_pixel(&self, x: u64, y: u64) -> u32 {
        if x >= self.framebuffer.width || y >= self.framebuffer.height {
            return 0;
        }

        let offset = y
            .saturating_mul(self.framebuffer.pitch)
            .saturating_add(x.saturating_mul(self.bytes_per_pixel as u64)) as usize;
        let mut color = 0u32;

        unsafe {
            let base = self.framebuffer.address.add(offset);
            for byte_index in 0..self.bytes_per_pixel {
                let byte = read_volatile(base.add(byte_index)) as u32;
                color |= byte << (byte_index * 8);
            }
        }

        color
    }

    fn pack_rgb(&self, red: u8, green: u8, blue: u8) -> u32 {
        pack_channel(red, self.framebuffer.red_mask_size, self.framebuffer.red_mask_shift)
            | pack_channel(
                green,
                self.framebuffer.green_mask_size,
                self.framebuffer.green_mask_shift,
            )
            | pack_channel(
                blue,
                self.framebuffer.blue_mask_size,
                self.framebuffer.blue_mask_shift,
            )
    }
}

fn normalize_glyph_byte(byte: u8) -> u8 {
    match byte {
        b'a'..=b'z' => byte - 32,
        _ => byte,
    }
}

fn pack_channel(value: u8, size: u8, shift: u8) -> u32 {
    if size == 0 {
        return 0;
    }

    let max = (1u32 << size) - 1;
    let scaled = ((value as u32) * max + 127) / 255;
    scaled << shift
}

fn format_mode_line(buffer: &mut [u8; 32], width: u64, height: u64, bpp: u16) -> usize {
    let mut length = 0;
    length += append_decimal(buffer, length, width);
    length += append_byte(buffer, length, b'X');
    length += append_decimal(buffer, length, height);
    length += append_byte(buffer, length, b'X');
    length += append_decimal(buffer, length, bpp as u64);
    length
}

fn append_decimal(buffer: &mut [u8; 32], offset: usize, value: u64) -> usize {
    let mut digits = [0u8; 20];
    let mut value = value;
    let mut count = 0;

    if value == 0 {
        digits[0] = b'0';
        count = 1;
    } else {
        while value != 0 {
            digits[count] = b'0' + (value % 10) as u8;
            value /= 10;
            count += 1;
        }
    }

    for index in 0..count {
        buffer[offset + index] = digits[count - 1 - index];
    }

    count
}

fn append_byte(buffer: &mut [u8; 32], offset: usize, byte: u8) -> usize {
    buffer[offset] = byte;
    1
}

fn glyph(byte: u8) -> [u8; 7] {
    match byte {
        b'0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        b'1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        b'3' => [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e],
        b'4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        b'5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        b'6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        b'7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        b'9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        b'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        b'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        b'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        b'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0e],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        b'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        b'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        b'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        b'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        b'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        b'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1b, 0x11],
        b'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        b'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        b':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        b'-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'/' => [0x01, 0x01, 0x02, 0x04, 0x08, 0x10, 0x10],
        b'=' => [0x00, 0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00],
        b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        b'[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        b']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b'#' => [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
        b',' => [0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c, 0x08],
        b'?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        b'+' => [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
        _ => [0x1f, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x1f],
    }
}
