#![feature(asm, global_asm)]
#![no_std]
#![no_main]

extern crate kernel_api;

use kernel_api::KernelError;

use core::convert::TryFrom;
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

struct Writer;

impl Writer {
    fn output_char(&self, c: char) {
        putc(c);
    }
}

impl core::fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.output_char(c);
        }
        Ok(())
    }
}

#[no_mangle]
fn _start() -> ! {
    use core::fmt::Write;
    let mut writer = Writer {};

    write!(&mut writer, "Hello World: {}\n", 123);

    let res = unsafe {
        let res = do_syscall(0x11, 0, 0, 0, 0);
        KernelError::try_from(res)
            .expect("Unknown error code")
    };

    write!(&mut writer, "Syscall Result: {:?}", res);

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
