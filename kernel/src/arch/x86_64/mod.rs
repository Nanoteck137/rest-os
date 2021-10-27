//! This module handles all the x86_64 specific code

#![allow(dead_code)]

pub use page_table::{ PageTable, PageType };

use gdt::{ GDT, TSS };

use apic::Apic;

use alloc::boxed::Box;

mod page_table;
mod serial;
mod gdt;
mod interrupts;
pub mod pic;
mod syscall;
mod apic;

const MSR_FS_BASE:        u32 = 0xc0000100;
const MSR_GS_BASE:        u32 = 0xc0000101;
const MSR_KERNEL_GS_BASE: u32 = 0xc0000102;

const MSR_EFER:  u32 = 0xc0000080;
const MSR_STAR:  u32 = 0xc0000081;
const MSR_LSTAR: u32 = 0xc0000082;
const MSR_FMASK: u32 = 0xc0000084;

pub struct ArchInfo {
    gdt: Option<Box<GDT>>,
    tss: Option<Box<TSS>>,

    apic: Option<Box<Apic>>,
}

impl ArchInfo {
    pub fn new() -> Self {
        Self {
            gdt: None,
            tss: None,
            apic: None,
        }
    }

    pub fn set_kernel_stack(&mut self, kernel_stack: u64) {
        self.tss.as_mut().expect("No TSS Available").set_kernel_stack(kernel_stack);
    }

    pub fn apic(&mut self) -> &mut Box<Apic> {
        self.apic.as_mut().unwrap()
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, packed)]
pub struct Regs {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    r11: u64,
    r10: u64,
    r9:  u64,
    r8:  u64,
    rbp: u64,
    rdi: u64,
    rsi: u64,
    rdx: u64,
    rcx: u64,
    rbx: u64,
    rax: u64,
}

#[inline]
pub unsafe fn out8(address: u16, data: u8) {
    asm!("out dx, al", in("dx") address, in("al") data);
}

#[inline]
pub unsafe fn in8(address: u16) -> u8 {
    let value: u8;

    asm!("in al, dx", out("al") value, in("dx") address);

    value
}

#[inline]
pub unsafe fn read_cr2() -> u64 {
    let value: u64;

    asm!("mov rax, cr2", out("rax") value);

    value
}

#[inline]
pub unsafe fn read_cr3() -> u64 {
    let value: u64;

    asm!("mov rax, cr3", out("rax") value);

    value
}

#[inline]
pub unsafe fn write_cr3(value: u64) {
    asm!("mov cr3, rax", in("rax") value);
}

#[inline]
pub unsafe fn read_flags() -> u64 {
    let value: u64;
    asm!("pushfq
          pop {}", out(reg) value);
    value
}

#[inline]
pub unsafe fn rdmsr(msr: u32) -> u64 {
    let value_low: u32;
    let value_high: u32;

    asm!("rdmsr",
         out("edx") value_high,
         out("eax") value_low,
         in("ecx") msr);

    (value_high as u64) << 32 | value_low as u64
}

#[inline]
pub unsafe fn wrmsr(msr: u32, value: u64) {
    let value_low = (value & 0xffffffff) as u32;
    let value_high = ((value >> 32) & 0xffffffff) as u32;
    asm!("wrmsr",
         in("edx") value_high,
         in("eax") value_low,
         in("ecx") msr);
}

#[inline]
pub unsafe fn read_fs_base() -> u64 {
    rdmsr(MSR_FS_BASE)
}

#[inline]
pub unsafe fn write_fs_base(base: u64) {
    wrmsr(MSR_FS_BASE, base)
}

#[inline]
pub unsafe fn read_gs_base() -> u64 {
    rdmsr(MSR_GS_BASE)
}

#[inline]
pub unsafe fn write_gs_base(base: u64) {
    wrmsr(MSR_GS_BASE, base)
}

#[inline]
pub unsafe fn read_kernel_gs_base() -> u64 {
    rdmsr(MSR_KERNEL_GS_BASE)
}

#[inline]
pub unsafe fn write_kernel_gs_base(base: u64) {
    wrmsr(MSR_KERNEL_GS_BASE, base)
}

#[inline]
pub unsafe fn force_enable_interrupts() {
    asm!("sti");
}

#[inline]
pub unsafe fn force_disable_interrupts() {
    asm!("cli");
}

#[inline]
pub fn is_interrupts_enabled() -> bool {
    let flags = unsafe { read_flags() };

    // TODO(patirk): Move 0x200 to constant
    flags & 0x200 == 0x200
}

pub fn early_initialize() {
    serial::initialize();
    pic::initialize();
}

pub fn initialize() {
    gdt::initialize();
    interrupts::initialize();
    syscall::initialize();

    serial::set_initialized();

    apic::initialize();

    // Initialize the BSP
    initialize_core(core!().core_id());
}

fn initialize_core(core_id: u32) {
    unsafe {
        apic::initialize_core(core_id);
    }
}

pub fn debug_print_fmt(args: core::fmt::Arguments) {
    core!().without_interrupts(|| {
        serial::print_fmt(args);
    });
}
