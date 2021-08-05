//! Module to handle processes

use crate::arch::x86_64::Regs;

use alloc::string::String;

pub struct Thread {
    name: String,
    control_block: ThreadControlBlock,
}

#[repr(C, packed)]
struct ThreadControlBlock {
    regs: Regs,
    rip: u64,
    rsp: u64,
}

extern "C" {
    fn thread_switch(regs: &ThreadControlBlock);
}

impl Thread {
    pub fn create_kernel_thread(name: String, entry: u64, stack: &'static [u8])
        -> Self
    {
        let control_block = ThreadControlBlock {
            regs: Regs::default(),
            rip: entry,
            rsp: stack.as_ptr() as u64 + stack.len() as u64,
        };

        Self {
            name,
            control_block,
        }
    }

    pub unsafe fn switch_to(&self) {
        thread_switch(&self.control_block);
    }
}

global_asm!(r#"
.global thread_switch
thread_switch:
    mov rsp, QWORD PTR [rdi + 0x80]

    mov r15, QWORD PTR [rdi + 0x00]
    mov r14, QWORD PTR [rdi + 0x08]
    mov r13, QWORD PTR [rdi + 0x10]
    mov r12, QWORD PTR [rdi + 0x18]
    mov r11, QWORD PTR [rdi + 0x20]
    mov r10, QWORD PTR [rdi + 0x28]
    mov r9,  QWORD PTR [rdi + 0x30]
    mov r8,  QWORD PTR [rdi + 0x38]
    mov rbp, QWORD PTR [rdi + 0x40]
    // mov rdi, QWORD PTR [rdi + 0x48]
    mov rsi, QWORD PTR [rdi + 0x50]
    mov rdx, QWORD PTR [rdi + 0x58]
    mov rcx, QWORD PTR [rdi + 0x60]
    mov rbx, QWORD PTR [rdi + 0x68]
    mov rax, QWORD PTR [rdi + 0x70]

    // Push the rip
    push QWORD PTR [rdi + 0x78]

    // Now we can set push the value RDI needs
    push QWORD PTR [rdi + 0x48]
    // Pop the value to set RDI
    pop rdi

    ret
"#);
