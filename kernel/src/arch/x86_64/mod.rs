//! This module handles all the x86_64 specific code

#![allow(dead_code)]

pub mod page_table;
pub mod serial;

pub fn out8(address: u16, data: u8) {
    unsafe {
        asm!("out dx, al", in("dx") address, in("al") data);
    }
}

pub fn in8(address: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") address);
    }
    value
}

pub fn get_cr3() -> u64 {
    let value: u64;

    unsafe {
        asm!("mov rax, cr3", out("rax") value);
    }

    value
}

pub fn set_cr3(value: u64) {
    unsafe {
        asm!("mov cr3, rax", in("rax") value);
    }
}

pub fn initialize() {
    serial::initialize();
}

pub fn debug_print_fmt(args: core::fmt::Arguments) {
    serial::print_fmt(args);
}
