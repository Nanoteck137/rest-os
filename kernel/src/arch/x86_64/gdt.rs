//! Module to handle the GDT creation and loading

use crate::mm::PAGE_SIZE;

use alloc::boxed::Box;

#[repr(C, packed)]
struct GDTDescriptor {
    size: u16,
    offset: u64,
}

#[derive(Copy, Clone, Default, Debug)]
#[repr(C, packed)]
pub struct TSS {
    reserved: u32,
    rsp: [u64; 3],
    reserved2: u64,
    ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,
    iopb_offset: u16,
}

impl TSS {
    pub(super) fn set_kernel_stack(&mut self, kernel_stack: u64) {
        self.rsp[0] = kernel_stack;
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct TSSEntry {
    low: u64,
    high: u64,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct GDTEntry {
    limit0: u16,
    base0: u16,
    base1: u8,
    access: u8,
    limit1_flags: u8,
    base2: u8
}

impl GDTEntry {
    const fn new(limit: u32, base: u32, access: u8, flags: u8) -> Self {
        let limit0 = (limit & 0xffff) as u16;
        let limit1 = ((limit >> 16) & 0xf) as u8;

        let base0 = (base & 0xffff) as u16;
        let base1 = ((base >> 16) & 0xff) as u8;
        let base2 = ((base >> 24) & 0xff) as u8;

        let flags = flags & 0xf;

        let limit1_flags = flags << 4 | limit1;

        Self {
            limit0,
            base0,
            base1,
            access,
            limit1_flags,
            base2
        }
    }
}

#[repr(C, packed)]
pub struct GDT {
    null:        GDTEntry, // 0x00
    kernel_code: GDTEntry, // 0x08
    kernel_data: GDTEntry, // 0x10
    tss:         TSSEntry, // 0x18
    user_data:   GDTEntry, // 0x28
    user_code:   GDTEntry, // 0x30
}

extern "C" {
    fn load_gdt(gdt: &GDTDescriptor);
}

static CRITICAL_STACK: [u8; PAGE_SIZE * 2] = [0; PAGE_SIZE * 2];
static NORMAL_STACK:   [u8; PAGE_SIZE * 2] = [0; PAGE_SIZE * 2];
static TEST_STACK:     [u8; PAGE_SIZE * 2] = [0; PAGE_SIZE * 2];

pub(super) fn initialize() {
    assert!(core!().arch().gdt.is_none(),
            "GDT Already initalized for this core: {}", core!().core_id());
    assert!(core!().arch().tss.is_none(),
            "TSS Already initalized for this core: {}", core!().core_id());

    let mut tss = Box::new(TSS::default());
    tss.rsp[0] = TEST_STACK.as_ptr() as u64 + TEST_STACK.len() as u64;
    tss.ist[0] = CRITICAL_STACK.as_ptr() as u64 + CRITICAL_STACK.len() as u64;
    tss.ist[1] = NORMAL_STACK.as_ptr() as u64 + NORMAL_STACK.len() as u64;

    let tss_base = &*tss as *const _ as u64;
    let tss_low = 0x890000000000 | (((tss_base >> 24) & 0xff) << 56) |
        ((tss_base & 0xffffff) << 16) |
        (core::mem::size_of::<TSS>() as u64 - 1);
    let tss_high = tss_base >> 32;

    let gdt = Box::new(GDT {
        null: GDTEntry::new(0, 0, 0, 0),
        kernel_code: GDTEntry::new(0, 0, 0x9a, 0x0a),
        kernel_data: GDTEntry::new(0, 0, 0x92, 0x0a),
        tss: TSSEntry { low: tss_low, high: tss_high },
        user_data: GDTEntry::new(0, 0, 0xf2, 0x0a),
        user_code: GDTEntry::new(0, 0, 0xfa, 0x0a),
    });

    // Define the GDT descriptor so it points to the our custom GDT table
    let gdt_desc = GDTDescriptor {
        size: (core::mem::size_of::<GDT>() - 1) as u16,
        offset: &*gdt as *const _ as u64
    };

    core!().arch().gdt = Some(gdt);
    core!().arch().tss = Some(tss);

    // Load the GDT
    unsafe {
        load_gdt(&gdt_desc);
    }
}

global_asm!(r#"
.global
load_gdt:
    // Load the GDT
    lgdt [rdi]

    // Setup the segments
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov ss, ax

    mov ax, 0x18
    ltr ax

    // Setup the code segment by far jumping to the return address
    pop rdi
    // Offset to the code segment inside the GDT
    mov rax, 0x08

    // Push the "arguments" for retfq
    push rax
    push rdi

    // Return to the return address
    retfq
"#);
