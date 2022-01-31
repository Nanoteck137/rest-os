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
use boot::{ BootInfo, BootMemoryMapType };

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

fn display_memory_map(boot_info: &BootInfo) {
    let mut availble_memory = 0;

    println!("Memory Map:");
    for entry in boot_info.memory_map() {
        let start = entry.addr().raw();
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

        if entry.typ() == BootMemoryMapType::Available {
            availble_memory += length;
        }

        tprintln!();
    }

    println!("Available memory: {}MiB", availble_memory / 1024 / 1024);
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size())
}

#[global_allocator]
static ALLOCATOR: Locked<Allocator> = Locked::new(Allocator::new());

// Linker variables
extern {
    static _heap_start: u32;
    static _heap_end: u32;
}

fn heap() -> (VirtualAddress, usize) {
    // The start of the heap is at the end of the kernel image and we get a
    // reference to that from the linker script
    let heap_start =
        unsafe { VirtualAddress(&_heap_start as *const u32 as usize) };
    let heap_end =
        unsafe { VirtualAddress(&_heap_end as *const u32 as usize) };

    let heap_size = heap_end.0 - heap_start.0;

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
        println!("Kernel Test thread");
    }
}

#[no_mangle]
pub extern fn kernel_init(boot_info_addr: u64) -> ! {
    unsafe {
        arch::force_disable_interrupts();
    }
    arch::early_initialize();

    let boot_info =
        unsafe { &*(boot_info_addr as *const u64 as *const BootInfo) };

    println!("{}", banner!());

    // Initialize the kernel heap
    initialize_heap();

    // Display the memory map from the bootloader
    display_memory_map(&boot_info);

    // Initialize the memory manager
    mm::initialize(&boot_info);

    // Initialize the BSP
    processor::init(0);

    // Initialize the time
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

    // Switch from the print buffer
    print::switch_early_print();
    print::console_init();
    print::flush_early_print_buffer();

    // Get a new reference to the boot info because the memory address has
    // moved when we initialized the memory manager
    let boot_info_addr = PhysicalAddress(boot_info_addr.try_into().unwrap());
    let boot_info_addr_virt =
        KERNEL_PHYSICAL_MEMORY.translate(boot_info_addr)
            .expect("Failed to translate boot info address");

    let boot_info =
        unsafe { &*(boot_info_addr_virt.0 as *const u64 as *const BootInfo) };

    // Initialize ACPI
    acpi::initialize(&KERNEL_PHYSICAL_MEMORY, &boot_info);

    // Initialize the arch
    arch::initialize();

    // Dump all the ACPI tables
    acpi::debug_dump();

    time::sleep(2 * 1000 * 1000);

    // Enable interrupts
    unsafe {
        core!().enable_interrupts();
    }

    core!().without_interrupts(|| {
        println!("Interrupts: {}", core!().is_interrupts_enabled());
    });

    {
        let initrd_addr = boot_info.initrd_addr().raw();
        let initrd_len: usize = boot_info.initrd_length().try_into().unwrap();

        let initrd_paddr = PhysicalAddress(initrd_addr.try_into().unwrap());
        let data = unsafe { KERNEL_PHYSICAL_MEMORY.slice(initrd_paddr, initrd_len) };
        let initrd_vaddr = KERNEL_PHYSICAL_MEMORY.translate(initrd_paddr)
            .expect("Failed to translate initrd address");

        if u16::from_le_bytes(data[0..2].try_into().unwrap()) == 0o070707 {
            // Binary cpio
            println!("Binary cpio");

            let cpio = CPIO::new(initrd_vaddr, initrd_len, CPIOKind::Binary);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070707" {
            println!("Odc cpio");

            let cpio = CPIO::new(initrd_vaddr, initrd_len, CPIOKind::Odc);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070701" {
            println!("Newc cpio");

            let cpio = CPIO::new(initrd_vaddr, initrd_len, CPIOKind::Newc);
            *CPIO.lock() = Some(cpio);
        } else if &data[0..6] == b"070702" {
            println!("Newc CRC cpio");

            let cpio = CPIO::new(initrd_vaddr, initrd_len, CPIOKind::Crc);
            *CPIO.lock() = Some(cpio);
        }
    }

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
