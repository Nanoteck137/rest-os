//! This is the main kernel file this has the kernel initializing code and
//! this is where the boot code is gonna call into.

#![feature(panic_info_message, const_mut_refs, alloc_error_handler)]
#![feature(asm, global_asm, get_mut_unchecked, const_btree_new)]
#![no_std]

// TODO(patrik): Temporary
#![allow(dead_code, unused_imports)]

/// TODO(patrik):
///  Currenly working on:
///
/// ---------------------------------------------------------------------------
///   - Go through and cleanup some error handling code
///   - Go through the code and fix all the locks so they behave
///     like they should with interrupts
///   - Switch from using the Spin crate to custom Locks so we can
///     handle interrupts enable/disable
///   - Processes
///     - Standard System calls
///     - 'replace_image'
///       - Change the stack start
///       - Change the initial stack size
///       - When we replace a kernel task then we need to figure out when
///         and how to free up the stack but we need to be careful because the
///         current executing thread chould be using the stack so that stack
///         free need to be defered
///   - File System
///     - Virtual File System
///     - FAT32 File System
///   - Device Handling
///     - AHCI
///     - How should the OS handle devices
///       - USB Devices
///       - PCI Devices
///   - Memory Manager
///     - Refactor the code
///       - Make the page table copy the kernel table entries inside
///         'page_fault_vmalloc'
///   - Arch
///     - Add Arm64 support?
///     - Add support for APIC
///       - Bring up more cores
///       - IO APIC
///   - ACPI Parsing
///   - Bugs
///

macro_rules! verify_interrupts_disabled {
    () => {{
        use crate::arch;
        assert!(!arch::is_interrupts_enabled(),
                "verify_interrupts_disabled: failed");
    }}
}

// Pull in the `alloc` create
#[macro_use] extern crate alloc;
/// Pull in the kernel api crate
#[macro_use] extern crate kernel_api;
#[macro_use] extern crate bitflags;

extern crate elf;
extern crate boot;

/// Poll in all the modules that the kernel has
#[macro_use] mod print;
#[macro_use] mod processor;
mod arch;
mod util;
mod multiboot;
mod mm;
mod thread;
mod process;
mod scheduler;
mod cpio;
mod acpi;
mod time;

use core::panic::PanicInfo;
use core::alloc::Layout;
use core::convert::TryInto;
use core::sync::atomic::{ AtomicUsize, Ordering };
use alloc::vec::Vec;
use alloc::sync::Arc;
use alloc::string::String;

use spin::{ Mutex, RwLock };

use util::Locked;
use mm::{ PhysicalMemory, VirtualAddress, PhysicalAddress };
use mm::{ Allocator, BitmapFrameAllocator };
use mm::{ BOOT_PHYSICAL_MEMORY, KERNEL_PHYSICAL_MEMORY };
use multiboot::{ Multiboot, Tag, MemoryMapEntryType };
use process::{ Process };
// use process::Task;
use scheduler::Scheduler;
use cpio::{ CPIO, CPIOKind };
use elf::{ Elf, ProgramHeaderType };
use boot::BootInfo;

use arch::x86_64::{ PageTable, PageType };

pub trait Device: Sync + Send {
    fn ioctl(&mut self, request: usize, arg0: usize, arg1: usize);
    fn write(&mut self, buffer: VirtualAddress, size: usize);
}

struct SerialDevice {
    ioctl_count: usize,
}

impl SerialDevice {
}

impl Device for SerialDevice {
    fn ioctl(&mut self, request: usize, arg0: usize, arg1: usize) {
        match request {
            0x01 => {
                let ptr = arg0 as *mut usize;
                unsafe {
                    core::ptr::write(ptr, 0x1337);
                }
            },

            _ => panic!("Bad request"),
        }
    }

    fn write(&mut self, buffer: VirtualAddress, size: usize) {
        let buffer = unsafe {
            core::slice::from_raw_parts(buffer.0 as *const u8, size)
        };

        for b in buffer {
            tprint!("{}", *b as char);
        }
    }
}

struct DummyDevice;

impl Device for DummyDevice {
    fn ioctl(&mut self, _request: usize, _arg0: usize, _arg1: usize) {
        println!("Dummy Device IOCTL");
    }

    fn write(&mut self, _buffer: VirtualAddress, _size: usize) {
        println!("Dummy Device WRITE");
    }
}

macro_rules! version {
    () => (env!("CARGO_PKG_VERSION"))
}

macro_rules! toolchain {
    () => (env!("RUSTUP_TOOLCHAIN"))
}

macro_rules! banner {
    () => (concat!("RestOS Version ", version!(), " ", toolchain!()))
}

fn display_memory_map(multiboot: &Multiboot) {
    let memory_map = multiboot.find_memory_map()
        .expect("Failed to find memory map");

    let mut availble_memory = 0;

    println!("Memory Map:");
    for entry in memory_map.iter() {
        let start = entry.addr();
        let length = entry.length();
        let end = start + length - 1;

        print!("[0x{:016x}-0x{:016x}] ", start, end);

        if length >= 1 * 1024 * 1024 * 1024 {
            tprint!("{:>4} GiB ", length / 1024 / 1024 / 1024);
        } else if length >= 1 * 1024 * 1024 {
            tprint!("{:>4} MiB ", length / 1024 / 1024);
        } else if length >= 1 * 1024 {
            tprint!("{:>4} KiB ", length / 1024);
        } else {
            tprint!("{:>4} B   ", length);
        }

        tprint!("{:?}", entry.typ());

        if entry.typ() == MemoryMapEntryType::Available {
            availble_memory += length;
        }

        tprintln!();
    }

    println!("Available memory: {}MiB", availble_memory / 1024 / 1024);
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
                println!("ACPI 1.0: {:?}", addr),

            Tag::Acpi2(addr, length) =>
                println!("ACPI 2.0: {:?} - {}", addr, length),

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

fn kernel_test_thread() {
    loop {
        // println!("Kernel Test thread");
    }
}

#[no_mangle]
pub extern fn kernel_init(boot_info_addr: u64) -> ! {
    arch::early_initialize();

    let boot_info =
        unsafe { &*(boot_info_addr as *const u64 as *const BootInfo) };

    println!("{}", banner!());

    println!("Kernel Boot: {:?}", boot_info);

    let multiboot_addr = 0;
    loop {}

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

    time::initialize();

    let serial_device = SerialDevice {
        ioctl_count: 0,
    };

    let dummy_device = DummyDevice {};

    /*
    let mut register_device = |name, device| {
        devices.insert(name, RwLock::new(device));
    };
    */

    register_device(String::from("serial_device_00"), Box::new(serial_device));
    register_device(String::from("dummy_device"), Box::new(dummy_device));

    print::switch_early_print();
    print::console_init();
    print::flush_early_print_buffer();

    acpi::initialize(&BOOT_PHYSICAL_MEMORY, &multiboot);
    arch::initialize();

    acpi::debug_dump();

    time::sleep(2 * 1000 * 1000);

    unsafe {
        core!().enable_interrupts();
    }

    core!().without_interrupts(|| {
        println!("Interrupts: {}", core!().is_interrupts_enabled());
    });

    multiboot.modules(|m| {
        println!("Module");
        let data = unsafe { m.data(&KERNEL_PHYSICAL_MEMORY) };

        let addr = VirtualAddress(data.as_ptr() as usize);
        let size = data.len();

        if u16::from_le_bytes(data[0..2].try_into().unwrap()) == 0o070707 {
            // Binary cpio
            println!("Binary cpio");

            let cpio = CPIO::new(addr, size, CPIOKind::Binary);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070707" {
            println!("Odc cpio");

            let cpio = CPIO::new(addr, size, CPIOKind::Odc);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070701" {
            println!("Newc cpio");

            let cpio = CPIO::new(addr, size, CPIOKind::Newc);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070702" {
            println!("Newc CRC cpio");

            let cpio = CPIO::new(addr, size, CPIOKind::Crc);
            *CPIO.lock() = Some(cpio);
        }
    });

    use alloc::borrow::ToOwned;

    let init_process = Process::create_kernel("Kernel Init".to_owned(),
                                              kernel_init_thread);

    Scheduler::add_process(init_process);

    let test_process = Process::create_kernel("Test Process".to_owned(),
                                              kernel_test_thread);
    Scheduler::add_process(test_process);

    Scheduler::debug_dump();

    unsafe {
        core!().scheduler().start();
    };
}

use alloc::boxed::Box;
use alloc::collections::BTreeMap;

type DeviceLock = Arc<Mutex<RwLock<Box<dyn Device>>>>;
static DEVICES: Mutex<RwLock<BTreeMap<String, DeviceLock>>> =
    Mutex::new(RwLock::new(BTreeMap::new()));

pub fn register_device(name: String, device: Box<dyn Device>) {
    let lock = DEVICES.lock();
    let mut lock = lock.write();

    lock.insert(name, Arc::new(Mutex::new(RwLock::new(device))));
}

pub fn find_device(name: &str) -> Option<DeviceLock> {
    let lock = DEVICES.lock();

    if let Some(device) = lock.read().get(name) {
        return Some(device.clone());
    }

    None
}

fn kernel_init_thread() {
    // TODO(patrik): Here we can release the stack we used from the bootloader.

    println!("kernel_init_thread: Hello World");

    {
        let serial = find_device("serial_device_00")
            .expect("Failed to find serial device");

        println!("Count: {}", Arc::strong_count(&serial));

        let lock = serial.lock();
        let mut lock = lock.write();

        let str = "Found the serial device printing\n";
        let addr = VirtualAddress(str.as_ptr() as usize);
        lock.write(addr, str.len());
    }

    core!().scheduler().set_ready();

    // let file = fs::open("/init");
    // let data = fs::read(file);
    unsafe {
        process::replace_image_exec(String::from("init"));
    }

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        // arch::x86_64::pic::disable();
        arch::force_disable_interrupts();
    }

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
