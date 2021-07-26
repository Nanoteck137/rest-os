//! This module handles all the x86_64 specific code

pub mod page_table;
pub mod serial;

fn out8(address: u16, data: u8) {
    unsafe {
        asm!("out dx, al", in("dx") address, in("al") data);
    }
}

fn in8(address: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") address);
    }
    value
}

pub fn initialize() {
    serial::initialize();
}

pub fn debug_print_fmt(args: core::fmt::Arguments) {
    serial::print_fmt(args);
}
