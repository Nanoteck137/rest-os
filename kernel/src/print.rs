//! This module handles printing to the user
//! For now we only have one way to show the user infomation
//! and that is the serial port and that is a good starting point
//! because QEMU can print the serial port to the terminal but
//! in the future we want to show the user the infomation on
//! the display

use crate::arch;
use crate::mm::{ VirtualAddress, PAGE_SIZE };

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::print::_print_fmt(format_args!($($arg)*));
    }}
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
        $crate::print!("\x1b[31m{}\x1b[0m", format_args!($($arg)*))
    }}
}

// Print macro that appends a newline to the end of a print
#[macro_export]
macro_rules! eprintln {
    () => ($crate::eprint!("\n"));
    ($($arg:tt)*) => ($crate::eprint!("{}\n", format_args!($($arg)*)))
}

const EARLY_PRINT_BUFFER_SIZE: usize = 2 * PAGE_SIZE;

struct EarlyPrintBuffer {
    len: usize,
    buffer: [u8; EARLY_PRINT_BUFFER_SIZE],
}

static mut EARLY_PRINT_BUFFER: EarlyPrintBuffer = EarlyPrintBuffer {
    len: 0,
    buffer: [0; EARLY_PRINT_BUFFER_SIZE],
};

impl core::fmt::Write for EarlyPrintBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            if self.len >= self.buffer.len() {
                // TODO(patrik): How should this be handled?
                break;
            }

            self.buffer[self.len] = b;
            self.len += 1;
        }

        Ok(())
    }
}

static mut USE_EARLY_PRINTING: bool = true;
static mut CONSOLE: Option<crate::DeviceLock> = None;

pub fn switch_early_print() {
    unsafe {
        USE_EARLY_PRINTING = false;
    }
}

pub fn flush_early_print_buffer() {
    let console = unsafe {
        (&CONSOLE).as_ref().unwrap().clone()
    };

    let lock = console.lock();
    let mut lock = lock.write();

    let (addr, len) = unsafe {
        let addr = EARLY_PRINT_BUFFER.buffer.as_ptr() as usize;
        let len = EARLY_PRINT_BUFFER.len;
        (VirtualAddress(addr), len)
    };

    lock.write(addr, len);
}

pub fn console_init() {
    let device = crate::find_device("serial_device_00");
    unsafe {
        CONSOLE = device;
    }
}

pub fn _print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    // TODO(patrik): Print to a side buffer when early printing then switch
    // to printing to a console device

    // TODO(patrik): Implement printing to temporary early buffer
    // TODO(patrik): Print to console device when early printing is out

    if unsafe { USE_EARLY_PRINTING } {
        // NOTE(patrik): We can use static mutable here because we know that
        // only one core is gonna access it
        unsafe {
            let _ = EARLY_PRINT_BUFFER.write_fmt(args);
        }
    } else {
        arch::debug_print_fmt(args);
    }
}
