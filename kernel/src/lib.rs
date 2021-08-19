//! This is the main kernel file this has the kernel initializing code and
//! this is where the boot code is gonna call into.

#![feature(panic_info_message, const_mut_refs, alloc_error_handler)]
#![feature(asm, global_asm, get_mut_unchecked)]
#![no_std]

// TODO(patrik): Temporary
#![allow(dead_code, unused_imports)]

/// Poll in all the modules that the kernel has
#[macro_use] mod print;
#[macro_use] mod processor;
mod arch;
mod util;
mod multiboot;
mod mm;
mod process;
mod scheduler;
mod cpio;
mod elf;

// Pull in the `alloc` create
#[macro_use] extern crate alloc;
/// Pull in the kernel api crate
#[macro_use] extern crate kernel_api;

use core::panic::PanicInfo;
use core::alloc::Layout;
use core::convert::TryInto;
use alloc::vec::Vec;
use alloc::sync::Arc;
use alloc::string::String;

use spin::{ Mutex, RwLock };

use util::Locked;
use mm::{ PhysicalMemory, VirtualAddress, PhysicalAddress };
use mm::{ Allocator, BitmapFrameAllocator };
use mm::{ BOOT_PHYSICAL_MEMORY, KERNEL_PHYSICAL_MEMORY };
use multiboot::{ Multiboot, Tag};
use process::{ Thread, Process };
use scheduler::Scheduler;
use cpio::CPIO;
use elf::{ Elf, ProgramHeaderType };

use arch::x86_64::{ PageTable, PageType };

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

fn _display_multiboot_tags(multiboot: &Multiboot) {
    for tag in multiboot.tags() {
        match tag {
            Tag::CommandLine(s) => println!("Command Line: {}", s),
            Tag::BootloaderName(s) => println!("Bootloader Name: {}", s),
            Tag::Module(m) => println!("Module: {:#x?}", m),

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

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size())
}

#[global_allocator]
static ALLOCATOR: Locked<Allocator> = Locked::new(Allocator::new());

// Linker variables
extern {
    static _end: u32;
}

fn get_kernel_end() -> VirtualAddress {
    unsafe { VirtualAddress(&_end as *const u32 as usize) }
}

fn heap() -> (VirtualAddress, usize) {
    // The start of the heap is at the end of the kernel image and we get a
    // reference to that from the linker script
    let heap_start = get_kernel_end();
    // For now we have 1 MiB of heap we could add more if we need more
    let heap_size = 1 * 1024 * 1024;

    (heap_start, heap_size)
}

fn initialize_heap() {
    let (heap_start, heap_size) = heap();

    unsafe {
        // Initialize the allocator
        ALLOCATOR.lock().init(heap_start, heap_size);
    }
}

unsafe fn allocate_memory(size: usize) -> VirtualAddress {
    ALLOCATOR.lock().alloc_memory(Layout::from_size_align(size, 8).unwrap())
        .expect("Failed to allocate memory")
}

static THREAD_STACK: [u8; 4096 * 4] = [0; 4096 * 4];

static CPIO: Mutex<Option<CPIO>> = Mutex::new(None);

pub fn read_initrd_file(path: String) -> Option<(*const u8, usize)> {
    let data = unsafe {
        let lock = CPIO.lock();
        let lock = lock.as_ref().unwrap();
        let slice = lock.read_file(path)?;

        (slice.as_ptr(), slice.len())
    };

    Some(data)
}

#[no_mangle]
extern fn kernel_init(multiboot_addr: usize) -> ! {
    arch::early_initialize();

    // Clear the display
    let ptr = 0xb8000 as *mut u16;
    unsafe {
        for i in 0..25*80 {
            *ptr.offset(i) = 0x0000;
        }
    }

    // Initialize the kernel heap
    initialize_heap();

    // Get access to the multiboot structure
    let multiboot = unsafe {
        Multiboot::from_addr(&BOOT_PHYSICAL_MEMORY,
                             PhysicalAddress(multiboot_addr))
    };

    // _display_multiboot_tags(&multiboot);

    // Display the memory map from the multiboot structure
    display_memory_map(&multiboot);

    let cmd_line = multiboot.find_command_line();
    if let Some(s) = cmd_line {
        println!("Kernel Command Line: {}", s);
    }

    mm::initialize(PhysicalAddress(multiboot_addr));
    processor::init(0);

    arch::initialize();

    multiboot.modules(|m| {
        let data = unsafe { m.data(&KERNEL_PHYSICAL_MEMORY) };

        if u16::from_le_bytes(data[0..2].try_into().unwrap()) == 0o070707 {
            // Binary cpio
            println!("Binary cpio");

            let addr = VirtualAddress(data.as_ptr() as usize);
            let size = data.len();
            let cpio = CPIO::binary(addr, size);
            {
                *CPIO.lock() = Some(cpio);
            }

            /*
            unsafe {
                let data = cpio.read_file(String::from("init"))
                    .expect("Failed to read the init file");

                let elf = Elf::parse(data)
                    .expect("Failed to parse 'init'");

                for program_header in elf.program_headers() {
                    if program_header.typ() == ProgramHeaderType::Load {
                        println!("Load: {:#x?}", program_header);

                        let data = elf.program_data(&program_header);
                        println!("Data: {:#x?}", data);
                        let size = program_header.memory_size() as usize;
                        mm::map_in_userspace(program_header.vaddr(), size)
                            .expect("Failed to map in userspace");

                        let source = data.as_ptr();
                        let dest = program_header.vaddr().0 as *mut u8;
                        let count = size;
                        core::ptr::copy_nonoverlapping(source, dest, count);
                    }
                }

                // println!("Elf: {:#x?}", elf);
            }
                */
        }
    });

    use alloc::borrow::ToOwned;

    let process = Process::create_kernel_process("Kernel Init".to_owned(),
                                                 kernel_init_thread);

    Scheduler::add_process(process);
    Scheduler::debug_dump_processes();

    unsafe {
        core!().scheduler().next();
    }

    panic!("Should not be here!!!");
}

fn kernel_init_thread() {
    // TODO(patrik): Here we can release the stack we used from the bootloader.

    println!("kernel_init_thread: Hello World");
    // println!("Current Process: {:#x?}", core!().process());

    unsafe {
        asm!("mov al, 0xff
            out 0xa1, al
            out 0x21, al");
    }

    // TODO(patrik):
    scheduler::replace_process_image(String::from("init"));

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
