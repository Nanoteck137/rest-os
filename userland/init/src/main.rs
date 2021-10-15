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

static mut WRITER: Writer = Writer {};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        _print_fmt(format_args!($($arg)*));
    }}
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

pub fn _print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    unsafe { WRITER.write_fmt(args).unwrap() };
}

#[no_mangle]
fn _start() -> ! {
    println!("Hello World: {}", 123);

    let res = unsafe {
        let mut value = 0u64;

        let ptr = &mut value as *mut _;
        let addr = ptr as u64;
        println!("Ptr: {:?}", ptr);

        println!("Before: {:#x}", value);
        let res = do_syscall(0x11, addr, 0, 0, 0);
        println!("After: {:#x}", value);
        KernelError::try_from(res)
            .expect("Unknown error code")
    };

    println!("Syscall Result: {:?}", res);

    loop {
        println!("Init Process");
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("Userland Init Panic");
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
