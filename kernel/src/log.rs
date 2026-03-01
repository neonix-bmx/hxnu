use core::fmt;
use core::fmt::Write;

use crate::serial::SerialWriter;
use crate::time;

#[allow(dead_code)]
pub fn write(args: fmt::Arguments<'_>) {
    let mut writer = SerialWriter::new();
    let _ = writer.write_fmt(args);
}

pub fn write_record(args: fmt::Arguments<'_>) {
    let mut writer = SerialWriter::new();
    let timestamp = time::timestamp();
    let _ = write!(writer, "[{}.{:09}] ", timestamp.seconds, timestamp.nanoseconds);
    let _ = writer.write_fmt(args);
    let _ = writer.write_str("\n");
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
