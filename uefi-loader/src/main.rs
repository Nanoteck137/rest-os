#![no_std]
#![no_main]

extern crate elf;

use core::panic::PanicInfo;

use efi::{ EfiHandle, EfiSystemTablePtr };
use elf::{ Elf, ProgramHeaderType };

mod efi;

struct ConsoleWriter {}

impl ConsoleWriter {
    fn print_str(&self, s: &str) {
        let mut buffer = [0u16; 1024];
        let mut index = 0;

        for c in s.bytes() {
            if c == b'\n' {
                buffer[index] = b'\r' as u16;
                index += 1;

                // TODO(patrik): Check 'p' for overflow and flush the buffer

                buffer[index] = b'\n' as u16;
                index += 1;

                // TODO(patrik): Check 'p' for overflow and flush the buffer

                continue;
            }

            buffer[index] = c as u16;
            index += 1;

            if index >= buffer.len() {
                // TODO(patrik): Flush the buffer
            }
        }

        // TODO(patrik): What to do here when an error occur?
        let _ = efi::output_string(&buffer);
    }
}

impl core::fmt::Write for ConsoleWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.print_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print_fmt(format_args!($($arg)*));
    }}
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

static mut WRITER: ConsoleWriter = ConsoleWriter {};

pub fn _print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    unsafe {
        let _ = WRITER.write_fmt(args);
    }
}

static KERNEL_BIN: &'static [u8] = include_bytes!("../../target/kernel.elf");

#[no_mangle]
fn efi_main(_image_handle: EfiHandle, table: EfiSystemTablePtr) -> ! {
    unsafe {
        table.register();
    }

    // TODO(patrik): Have a copy of the kernel.elf inside this executable
    // TODO(patrik): Setup the kernel page table
    // TODO(patrik): Load in the kernel
    // TODO(patrik): Load the initrd
    // TODO(patrik): Create some kind of structure to pass in to the kernel
    //   - Memory map
    //   - ACPI Tables
    //   - Kernel command line, Where from to retrive the command line?
    //     - Read from a file?
    //     - Embed inside the bootloader or kernel executable?
    //   - Initrd

    efi::clear_screen()
        .expect("Failed to clear the screen");

    let elf = Elf::parse(&KERNEL_BIN)
        .expect("Failed to parse kernel elf");
    for program_header in elf.program_headers() {
        println!("Program Header: {:#x?}", program_header);

        if program_header.typ() != ProgramHeaderType::Load {
            continue;
        }

        let memory_size = program_header.memory_size();
        let page_count = memory_size / 0x1000 + 1;
        let page_count = page_count as usize;
        println!("Needs {} pages", page_count);

        let addr = efi::allocate_pages(page_count)
            .expect("Failed to allocate pages");
        println!("Allocated address: {:#x}", addr);

        let ptr = addr as *mut u8;

        unsafe {
            // Zero out the allocated region
            core::ptr::write_bytes(ptr, 0, page_count * 0x1000);
        }

        let data = elf.program_data(&program_header);
        let data_size = program_header.file_size() as usize;

        unsafe {
            // Copy the bytes from the program header to the allocated region
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data_size);
        }
    }

    println!("ELF: {:?}", core::str::from_utf8(&KERNEL_BIN[0..4]));
    println!("Hello World: {:#x?}", KERNEL_BIN.as_ptr());

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}
