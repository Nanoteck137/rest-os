//! Module to handle the GDT creation and loading

#[repr(C, packed)]
struct GDTDescriptor {
    size: u16,
    offset: u64,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct GDTEntry {
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
struct GDT {
    null: GDTEntry,
    kernel_code: GDTEntry,
    kernel_data: GDTEntry,
    user_code: GDTEntry,
    user_data: GDTEntry,
}

/// Define the GDT for the kernel
static GDT: GDT = GDT {
    null: GDTEntry::new(0, 0, 0, 0),
    kernel_code: GDTEntry::new(0, 0, 0x9a, 0x0a),
    kernel_data: GDTEntry::new(0, 0, 0x92, 0x0a),
    user_code: GDTEntry::new(0, 0, 0x9a, 0x0a),
    user_data: GDTEntry::new(0, 0, 0x9a, 0x0a),
};

extern "C" {
    fn load_gdt(gdt: &GDTDescriptor);
}

pub(super) fn initialize() {
    // Define the GDT descriptor so it points to the our custom GDT table
    let gdt = GDTDescriptor {
        size: (core::mem::size_of::<GDT>() - 1) as u16,
        offset: &GDT as *const _ as u64
    };

    // Load the GDT
    unsafe {
        load_gdt(&gdt);
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
    mov fs, ax
    mov gs, ax
    mov ss, ax

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
