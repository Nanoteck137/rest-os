#![feature(asm, panic_info_message)]
#![no_std]

mod serial;
#[macro_use] mod print;
mod mm;
mod multiboot;

use core::panic::PanicInfo;

use mm::{ PhysicalMemory, PhysicalAddress };
use multiboot::{ Multiboot, Tag};

const KERNEL_TEXT_START: usize = 0xffffffff80000000;
const KERNEL_TEXT_SIZE: usize = 1 * 1024 * 1024 * 1024;
const KERNEL_TEXT_END: usize = KERNEL_TEXT_START + KERNEL_TEXT_SIZE - 1;

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

fn display_memory_map(multiboot: &Multiboot) {
    let memory_map = multiboot.find_memory_map()
        .expect("Failed to find memory map");

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

fn display_multiboot_tags(multiboot: &Multiboot) {
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

            Tag::MemoryMap(_memory_map) => {
                println!("Memory Map Tag");
            }

            Tag::Framebuffer(framebuffer) =>
                println!("{:#?}", framebuffer),

            Tag::ElfSections(elf_sections) => {
                let table = elf_sections.string_table(&BOOT_PHYSICAL_MEMORY)
                    .expect("Failed to find the string table");

                for section in elf_sections.iter() {
                    println!("{} Section: {:#x?}",
                             table.string(section.name_index()).unwrap(),
                             section);
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
}

static BOOT_PHYSICAL_MEMORY: BootPhysicalMemory = BootPhysicalMemory {};

#[no_mangle]
extern fn kernel_init(multiboot_addr: usize) -> ! {
    serial::initialize();

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

    // display_multiboot_tags(&multiboot);
    display_memory_map(&multiboot);

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
