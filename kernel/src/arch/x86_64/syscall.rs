//! Module to initialize syscall usage

use super::Regs;
use super::{ rdmsr, wrmsr };
use super::{ MSR_EFER, MSR_STAR, MSR_LSTAR, MSR_FMASK };
use super::serial::SERIAL_PORT;

use kernel_api::KernelError;

extern "C" {
    fn syscall_entry();
}

pub(super) fn initialize() {
    let handler_addr = syscall_entry as *const () as u64;

    unsafe {
        let efer = rdmsr(MSR_EFER);
        wrmsr(MSR_EFER, efer | 1);

        wrmsr(MSR_FMASK, 0x002);
        wrmsr(MSR_LSTAR, handler_addr);

        let kernel_cs = 0x08u64;
        let user_cs = 0x20u64 | 3;

        let star = user_cs << 48 | kernel_cs << 32;
        wrmsr(MSR_STAR, star);
    }
}

#[no_mangle]
fn syscall_handler(regs: &mut Regs) {
    let number = regs.rax;
    let arg0 = regs.rdi;
    let arg1 = regs.rsi;
    let arg2 = regs.rdx;
    let arg3 = regs.r10;

    /*
    println!("Syscall Number: {}", number);
    println!("Syscall Arg0: {}", arg0);
    println!("Syscall Arg1: {}", arg1);
    println!("Syscall Arg2: {}", arg2);
    println!("Syscall Arg3: {}", arg3);

    println!("Regs: {:#?}", regs);
    */

    if number == 0x10 {
        SERIAL_PORT.lock().as_mut().unwrap().output_char(arg0 as u8 as char);
    }

    regs.rax = KernelError::TestError as u64;
}

global_asm!(r#"
.extern syscall_handler
.global syscall_entry
syscall_entry:
    swapgs

    mov gs:[0x10], rsp // Save the user stack
    mov rsp, gs:[0x08] // Setup the kernel stack

	push rax
	push rbx
	push rcx
	push rdx
	push rsi
	push rdi
	push rbp
	push r8
	push r9
	push r10
	push r11
	push r12
	push r13
	push r14
	push r15

	mov rax, 0
	mov rbx, 0
	mov rcx, 0
	mov rdx, 0
	mov rsi, 0
	mov rdi, 0
	mov rbp, 0
	mov r8, 0
	mov r9, 0
	mov r10, 0
	mov r11, 0
	mov r12, 0
	mov r13, 0
	mov r14, 0
	mov r15, 0

    mov rdi, rsp
    call syscall_handler

	pop r15
	pop r14
	pop r13
	pop r12
	pop r11
	pop r10
	pop r9
	pop r8
	pop rbp
	pop rdi
	pop rsi
	pop rdx
	pop rcx
	pop rbx
	pop rax

    mov rsp, gs:[0x10] // Restore the user stack

    swapgs
    sysretq
"#);
