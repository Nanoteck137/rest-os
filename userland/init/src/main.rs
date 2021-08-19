#![feature(asm, global_asm)]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

extern "C" {
    fn do_syscall(number: u64, arg0: u64, arg1: u64,
                  arg2: u64, arg3: u64) -> u64;
}

fn putc(c: char) {
    unsafe {
        do_syscall(0x10, c as u64, 0, 0, 0);
    }
}

#[no_mangle]
fn _start() -> ! {
    let s = "Hello World from init\n";
    for c in s.chars() {
        putc(c);
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

global_asm!(r#"
.global do_syscall
do_syscall:
    mov rax, rdi
    mov rdi, rsi
    mov rsi, rdx
    mov rdx, rcx
    mov r10, r8

    syscall

    ret
"#);
