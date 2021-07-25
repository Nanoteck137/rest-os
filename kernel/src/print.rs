//! This module handles printing to the user
//! For now we only have one way to show the user infomation
//! and that is the serial port and that is a good starting point
//! because QEMU can print the serial port to the terminal but
//! in the future we want to show the user the infomation on
//! the display

use crate::serial;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print_fmt(format_args!($($arg)*)))
}

// Print macro that appends a newline to the end of a print
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        $crate::print::ecolor_on();
        $crate::print::_print_fmt(format_args!($($arg)*));
        $crate::print::ecolor_off();
    }}
}

// Print macro that appends a newline to the end of a print
#[macro_export]
macro_rules! eprintln {
    () => ($crate::eprint!("\n"));
    ($($arg:tt)*) => ($crate::eprint!("{}\n", format_args!($($arg)*)))
}

pub fn ecolor_on() {
    print!("\x1b[1;31m");
}

pub fn ecolor_off() {
    print!("\x1b[0m");
}

pub fn _print_fmt(args: core::fmt::Arguments) {
    serial::print_fmt(args);
}
