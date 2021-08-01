//! This is the main kernel file this has the kernel initializing code and
//! this is where the boot code is gonna call into.

#![feature(asm, panic_info_message, const_mut_refs, alloc_error_handler)]
#![no_std]

/// Poll in all the modules that the kernel has
mod arch;
mod util;
#[macro_use] mod print;
mod mm;
mod multiboot;

// Pull in the `alloc` create
#[macro_use] extern crate alloc;

use core::panic::PanicInfo;

use util::Locked;
use mm::{ PhysicalMemory, VirtualAddress, PhysicalAddress };
use mm::heap_alloc::Allocator;
use mm::frame_alloc::BitmapFrameAllocator;
use multiboot::{ Multiboot, Tag};

use arch::x86_64::page_table::{ PageTable, PageType };

// NOTE(patrik): Almost the same as the Linux kernel
const KERNEL_TEXT_START: usize = 0xffffffff80000000;
const KERNEL_TEXT_SIZE:  usize = 1 * 1024 * 1024 * 1024;
const KERNEL_TEXT_END:   usize = KERNEL_TEXT_START + KERNEL_TEXT_SIZE - 1;

// NOTE(patrik): Same as the Linux kernel
const PHYSICAL_MEMORY_OFFSET:     usize = 0xffff888000000000;
const PHYSICAL_MEMORY_OFFSET_END: usize = 0xffff890000000000;

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

        let new_addr = paddr.0 + KERNEL_TEXT_START;
        core::slice::from_raw_parts(new_addr as *const T, size)
    }

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + KERNEL_TEXT_START;
        assert!(end <= KERNEL_TEXT_END,
                "Slicing address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START;
        core::slice::from_raw_parts_mut(new_addr as *mut T, size)
    }
}

struct KernelPhysicalMemory;

impl PhysicalMemory for KernelPhysicalMemory {
    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            PHYSICAL_MEMORY_OFFSET;
        assert!(end < PHYSICAL_MEMORY_OFFSET_END,
                "Reading address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_OFFSET;
        core::ptr::read_volatile(new_addr as *const T)
    }

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T) {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            PHYSICAL_MEMORY_OFFSET;
        assert!(end < PHYSICAL_MEMORY_OFFSET_END,
                "Writing address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_OFFSET;
        core::ptr::write_volatile(new_addr as *mut T, value)
    }

    // Read a slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + PHYSICAL_MEMORY_OFFSET;
        assert!(end < PHYSICAL_MEMORY_OFFSET_END,
                "Slicing address '{:?}' is over the physical memory area", paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_OFFSET;
        core::slice::from_raw_parts(new_addr as *const T, size)
    }

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + PHYSICAL_MEMORY_OFFSET;
        assert!(end < PHYSICAL_MEMORY_OFFSET_END,
                "Slicing address '{:?}' is over the physical memory area",
                paddr);
        let new_addr = paddr.0 + PHYSICAL_MEMORY_OFFSET;
        core::slice::from_raw_parts_mut(new_addr as *mut T, size)
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

fn _display_multiboot_tags(multiboot: &Multiboot) {
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

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size())
}

static BOOT_PHYSICAL_MEMORY: BootPhysicalMemory = BootPhysicalMemory {};
static KERNEL_PHYSICAL_MEMORY: KernelPhysicalMemory = KernelPhysicalMemory {};

#[global_allocator]
static ALLOCATOR: Locked<Allocator> = Locked::new(Allocator::new());

// Linker variables
extern {
    static _end: u32;
}

fn heap() -> (VirtualAddress, usize) {
    // The start of the heap is at the end of the kernel image and we get a
    // reference to that from the linker script
    let heap_start = unsafe { VirtualAddress(&_end as *const u32 as usize) };
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

#[no_mangle]
extern fn kernel_init(multiboot_addr: usize) -> ! {
    arch::initialize();

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

    // display_multiboot_tags(&multiboot);

    // Display the memory map from the multiboot structure
    display_memory_map(&multiboot);

    let cmd_line = multiboot.find_command_line();
    if let Some(s) = cmd_line {
        println!("Kernel Command Line: {}", s);
    }

    let (heap_start, heap_size) = heap();
    let heap_end = heap_start + heap_size;
    let physical_frame_start = PhysicalAddress(heap_end.0 - KERNEL_TEXT_START);
    let physical_frame_end =
        PhysicalAddress((heap_end.0 - KERNEL_TEXT_START) + 4096 * 10);

    let mut frame_allocator =
        BitmapFrameAllocator::new();
    unsafe {
        frame_allocator.init(multiboot.find_memory_map()
            .expect("Failed to find memory map"));
    }

    println!("Frame Allocator: {:#?}", frame_allocator);

    frame_allocator.lock_region(PhysicalAddress(0), 0x4000);

    use mm::frame_alloc::FrameAllocator;
    println!("Frame: {:?}", frame_allocator.alloc_frame());

    let cr3 = arch::x86_64::get_cr3();
    println!("CR3: {:#x}", cr3);

    let mut page_table =
        unsafe { PageTable::from_table(PhysicalAddress(cr3 as usize)) };

    unsafe {
        for offset in (0..8 * 1024 * 1024 * 1024).step_by(2 * 1024 * 1024) {
            let vaddr = VirtualAddress(offset + PHYSICAL_MEMORY_OFFSET);
            let paddr = PhysicalAddress(offset);
            page_table.map(&mut frame_allocator, &BOOT_PHYSICAL_MEMORY,
                           vaddr, paddr, PageType::Page2M)
                .expect("Failed to map");
        }
    }

    let multiboot = unsafe {
        Multiboot::from_addr(&KERNEL_PHYSICAL_MEMORY,
                             PhysicalAddress(multiboot_addr))
    };

    display_memory_map(&multiboot);

    // Debug print that we are done executing
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
