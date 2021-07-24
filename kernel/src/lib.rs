#![feature(asm, panic_info_message)]
#![no_std]

mod mm;
mod multiboot;

use core::panic::PanicInfo;
use spin::Mutex;

use mm::{ PhysicalMemory, VirtualAddress, PhysicalAddress };
use multiboot::{ Multiboot, Tag, MemoryMap };

const KERNEL_TEXT_START: usize = 0xffffffff80000000;
const KERNEL_TEXT_SIZE: usize = 1 * 1024 * 1024 * 1024;
const KERNEL_TEXT_END: usize = KERNEL_TEXT_START + KERNEL_TEXT_SIZE - 1;

fn out8(address: u16, data: u8) {
    unsafe {
        asm!("out dx, al", in("dx") address, in("al") data);
    }
}

fn in8(address: u16) -> u8 {
    let value: u8;
    unsafe {
        asm!("in al, dx", out("al") value, in("dx") address);
    }
    value
}

struct SerialPort {
    port: u16,
}

impl SerialPort {
    fn new(port: u16) -> Self {
        out8(port + 1, 0x00);    // Disable all interrupts
        out8(port + 3, 0x80);    // Enable DLAB (set baud rate divisor)
        out8(port + 0, 0x03);    // Set divisor to 3 (lo byte) 38400 baud
        out8(port + 1, 0x00);    //                  (hi byte)
        out8(port + 3, 0x03);    // 8 bits, no parity, one stop bit
        out8(port + 2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
        out8(port + 4, 0x0B);    // IRQs enabled, RTS/DSR set
        out8(port + 4, 0x0F);

        Self {
            port
        }
    }

    fn is_transmit_empty(&self) -> bool {
        return in8(self.port + 5) & 0x20 != 0;
    }

    fn output_char(&mut self, c: char) {
        while !self.is_transmit_empty() {}

        out8(self.port, c as u8);
    }
}

impl core::fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.output_char(c);
        }

        Ok(())
    }
}

static SERIAL_PORT: Mutex<Option<SerialPort>> = Mutex::new(None);

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print_fmt(format_args!($($arg)*)))
}

// Print macro that appends a newline to the end of a print
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        ecolor_on();
        $crate::_print_fmt(format_args!($($arg)*));
        ecolor_off();
    }}
}

// Print macro that appends a newline to the end of a print
#[macro_export]
macro_rules! eprintln {
    () => ($crate::eprint!("\n"));
    ($($arg:tt)*) => ($crate::eprint!("{}\n", format_args!($($arg)*)))
}

fn ecolor_on() {
    print!("\x1b[1;31m");
}

fn ecolor_off() {
    print!("\x1b[0m");
}

fn _print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    let mut lock = SERIAL_PORT.lock();
    match lock.as_mut() {
        Some(f) => {
            f.write_fmt(args).unwrap();
        }
        None => {
        }
    }
}

struct BootPhysicalMemory;

impl PhysicalMemory for BootPhysicalMemory {
    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) + KERNEL_TEXT_START;
        assert!(end <= KERNEL_TEXT_END,
                "Reading address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START;
        core::ptr::read_volatile(new_addr as *const T)
    }

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T) {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) + KERNEL_TEXT_START;
        assert!(end <= KERNEL_TEXT_END,
                "Writing address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START;
        core::ptr::write_volatile(new_addr as *mut T, value)
    }

    // Read a slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + KERNEL_TEXT_START;
        assert!(end <= KERNEL_TEXT_END,
                "Slicing address '{:?}' is over the kernel text area", paddr);
        core::slice::from_raw_parts(paddr.0 as *const T, size)
    }

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + KERNEL_TEXT_START;
        assert!(end <= KERNEL_TEXT_END,
                "Slicing address '{:?}' is over the kernel text area", paddr);
        core::slice::from_raw_parts_mut(paddr.0 as *mut T, size)
    }
}

fn display_memory_map(memory_map: MemoryMap) {
    println!("Memory Map:");
    for entry in memory_map.iter() {
        let start = entry.addr();
        let length = entry.length();
        let end = start + length - 1;

        print!("[0x{:016x}-0x{:016x}] ", start, end);

        if length >= 1 * 1024 * 1024 * 1024 {
            print!("{:>4} GiB ", length / 1024 / 1024 / 1024);
        } else if length >= 1 * 1024 * 1024 {
            print!("{:>4} MiB ", length / 1024 / 1024);
        } else if length >= 1 * 1024 {
            print!("{:>4} KiB ", length / 1024);
        } else {
            print!("{:>4} B   ", length);
        }

        print!("{:?}", entry.typ());

        println!();
    }
}

static BOOT_PHYSICAL_MEMORY: BootPhysicalMemory = BootPhysicalMemory {};

#[no_mangle]
extern fn kernel_init(multiboot_addr: usize) -> ! {
    {
        *SERIAL_PORT.lock() = Some(SerialPort::new(0x3f8));
    }

    let ptr = 0xb8000 as *mut u16;
    unsafe {
        for i in 0..25*80 {
            *ptr.offset(i) = 0x0000;
        }
    }

    let multiboot = unsafe {
        Multiboot::from_addr(&BOOT_PHYSICAL_MEMORY,
                             PhysicalAddress(multiboot_addr))
    };

    for tag in multiboot.tags() {
        match tag {
            Tag::CommandLine(s) => println!("Command Line: {}", s),
            Tag::BootloaderName(s) =>
                println!("Bootloader Name: {}", s),

            Tag::BasicMemInfo(lower, upper) =>
                println!("Basic Memory Info - Lower: {} Upper: {}",
                         lower, upper),

            Tag::BootDev(boot_dev) =>
                println!("Boot Device: {:#x?}", boot_dev),

            Tag::MemoryMap(memory_map) => {
                display_memory_map(memory_map);
            }

            Tag::Framebuffer(framebuffer) =>
                println!("{:#?}", framebuffer),

            Tag::ElfSections(elf_sections) => {
                let table = elf_sections.string_table(&BOOT_PHYSICAL_MEMORY)
                    .expect("Failed to find the string table");

                for section in elf_sections.iter() {
                    /*
                    println!("{} Section: {:#x?}",
                             table.string(section.name_index()).unwrap(),
                             section);
                    */
                }
            }

            Tag::Acpi1(addr) =>
                println!("ACPI 1.0: {:#x}", addr),

            Tag::Acpi2(addr) =>
                println!("ACPI 2.0: {:#x}", addr),

            Tag::LoadBaseAddr(addr) =>
                println!("Load Base Addr: {:#x}", addr),

            Tag::Unknown(index) =>
                eprintln!("Unknown index: {}", index),
        }
    }

    println!("Done");

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("---------------- KERNEL PANIC ----------------");
    // Print out the location of the panic
    if let Some(loc) = info.location() {
        println!("Location: {}:{}", loc.file(), loc.line());
    }

    // Print out the message of the panic
    if let Some(message) = info.message() {
        println!("Message: {}", message);
    }
    println!("----------------------------------------------");

    loop {}
}
