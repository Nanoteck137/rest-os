//! Module to handle the PIC (Intel 8259)

use super::out8;

const PIC1: u16 = 0x0020;
const PIC2: u16 = 0x00A0;

const PIC1_CMD:  u16 = PIC1;
const PIC1_DATA: u16 = PIC1 + 1;

const PIC2_CMD:  u16 = PIC2;
const PIC2_DATA: u16 = PIC2 + 1;

const PIC_CMD_EOI: u8 = 0x20;

const NUM_INTERRUPTS: u8 = 16;
const REMAP_BASE:     u8 = 32;

const ICW1_ICW4:      u8 = 0x01;
const ICW1_SINGLE:    u8 = 0x02;
const ICW1_INTERVAL4: u8 = 0x04;
const ICW1_LEVEL:     u8 = 0x08;
const ICW1_INIT:      u8 = 0x10;

const ICW4_8086:       u8 = 0x01;
const ICW4_AUTO:       u8 = 0x02;
const ICW4_BUF_SLAVE:  u8 = 0x08;
const ICW4_BUF_MASTER: u8 = 0x0C;
const ICW4_SFNM:       u8 = 0x10;

pub(super) fn initialize() {
    unsafe {
        out8(PIC1_CMD, ICW1_INIT | ICW1_ICW4);
        out8(PIC2_CMD, ICW1_INIT | ICW1_ICW4);
        out8(PIC1_DATA, REMAP_BASE);
        out8(PIC2_DATA, REMAP_BASE + 8);
        out8(PIC1_DATA, 4);
        out8(PIC2_DATA, 2);

        out8(PIC1_DATA, ICW4_8086);
        out8(PIC2_DATA, ICW4_8086);
    }

    disable();
}

pub fn enable(mask: u16) {
    let mask = !mask;

    let pic1_mask = (mask & 0xff) as u8;
    let pic2_mask = ((mask >> 8) & 0xff) as u8;

    unsafe {
        out8(PIC1_DATA, pic1_mask);
        out8(PIC2_DATA, pic2_mask);
    }
}

pub fn disable() {
    unsafe {
        out8(PIC1_DATA, 0xff);
        out8(PIC2_DATA, 0xff);
    }
}

pub fn send_eoi(int_number: u8) {
    if int_number >= REMAP_BASE && int_number < REMAP_BASE + NUM_INTERRUPTS {
        if int_number >= REMAP_BASE + 8 {
            // PIC2 EOI
            unsafe {
                out8(PIC2_CMD, PIC_CMD_EOI);
            }
        }

        // PIC1 EOI
        unsafe {
            out8(PIC1_CMD, PIC_CMD_EOI);
        }
    }
}
