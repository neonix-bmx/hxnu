use core::ptr::{read_volatile, write_volatile};

use crate::limine;

const LIMINE_FRAMEBUFFER_RGB: u8 = 1;

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
}

impl FramebufferError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedMemoryModel => "unsupported framebuffer memory model",
            Self::UnsupportedBitsPerPixel => "unsupported framebuffer bits-per-pixel",
            Self::MissingAddress => "framebuffer address is null",
        }
    }
}

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

    let mut writer = FramebufferWriter::new(framebuffer, bytes_per_pixel);
    let background = writer.pack_rgb(0x09, 0x11, 0x1b);
    let header = writer.pack_rgb(0x11, 0x2b, 0x44);
    let accent = writer.pack_rgb(0x2d, 0xd4, 0xbf);
    let accent_soft = writer.pack_rgb(0x79, 0xe2, 0xd0);
    let text = writer.pack_rgb(0xf5, 0xf7, 0xfa);

    writer.clear(background);
    writer.fill_rect(0, 0, framebuffer.width, 72, header);
    writer.fill_rect(0, 72, framebuffer.width, 4, accent);
    writer.fill_rect(32, 112, 256, 144, header);
    writer.fill_rect(48, 128, 32, 96, accent);
    writer.fill_rect(96, 128, 32, 96, accent_soft);
    writer.fill_rect(144, 128, 32, 96, accent);
    writer.fill_rect(192, 128, 32, 96, accent_soft);
    writer.fill_rect(240, 128, 32, 96, accent);

    writer.draw_text(32, 20, "HXNU 2605", text, 3);
    writer.draw_text(320, 32, "FB READY", text, 2);

    let mut mode_line = [0u8; 32];
    let mode_len = format_mode_line(&mut mode_line, framebuffer.width, framebuffer.height, framebuffer.bpp);
    writer.draw_text_bytes(320, 72, &mode_line[..mode_len], accent, 2);

    let sample_background = writer.read_pixel(8, 8);
    let sample_accent = writer.read_pixel(56, 136);

    Ok(FramebufferSummary {
        width: framebuffer.width,
        height: framebuffer.height,
        pitch: framebuffer.pitch,
        bpp: framebuffer.bpp,
        sample_background,
        sample_accent,
    })
}

struct FramebufferWriter {
    framebuffer: limine::Framebuffer,
    bytes_per_pixel: usize,
}

impl FramebufferWriter {
    const fn new(framebuffer: limine::Framebuffer, bytes_per_pixel: usize) -> Self {
        Self {
            framebuffer,
            bytes_per_pixel,
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
            self.draw_glyph(cursor_x, y, *byte, color, scale);
            cursor_x = cursor_x.saturating_add(6 * scale);
        }
    }

    fn draw_glyph(&mut self, x: u64, y: u64, byte: u8, color: u32, scale: u64) {
        let glyph = glyph(byte);
        for (row_index, row_bits) in glyph.iter().enumerate() {
            for column in 0..5 {
                if row_bits & (1 << (4 - column)) == 0 {
                    continue;
                }
                let pixel_x = x.saturating_add((column as u64) * scale);
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
        b'A' | b'a' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' | b'b' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'D' | b'd' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        b'E' | b'e' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' | b'f' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'H' | b'h' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'N' | b'n' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        b'R' | b'r' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        b'U' | b'u' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'X' | b'x' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        b'Y' | b'y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        b' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        _ => [0x1f, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x1f],
    }
}
