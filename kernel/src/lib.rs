#![feature(asm, panic_info_message, const_mut_refs, alloc_error_handler)]
#![no_std]

mod serial;
#[macro_use] mod print;
mod mm;
mod multiboot;

extern crate alloc;

use core::panic::PanicInfo;
use alloc::alloc::{ GlobalAlloc, Layout };

use mm::{ PhysicalMemory, VirtualAddress, PhysicalAddress };
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

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

struct AllocNode {
    size: usize,
    next: Option<&'static mut AllocNode>
}

impl AllocNode {
    const fn new(size: usize) -> Self {
        Self {
            size,
            next: None
        }
    }

    fn start_addr(&self) -> VirtualAddress {
        VirtualAddress(self as *const Self as usize)
    }

    fn end_addr(&self) -> VirtualAddress {
        self.start_addr() + self.size
    }
}

struct Allocator {
    head: AllocNode
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            head: AllocNode::new(0)
        }
    }

    pub unsafe fn init(&mut self,
                       heap_start: VirtualAddress,
                       heap_size: usize)
    {
        self.add_free_region(heap_start, heap_size);
    }

    unsafe fn add_free_region(&mut self, addr: VirtualAddress, size: usize) {
        assert_eq!(align_up(addr.0, core::mem::align_of::<AllocNode>()),
                   addr.0);
        assert!(size >= core::mem::size_of::<AllocNode>());

        let mut node = AllocNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr.0 as *mut AllocNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr);
    }

    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut AllocNode, VirtualAddress)>
    {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) =
                Self::alloc_from_region(&region, size, align)
            {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;

                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    fn alloc_from_region(region: &AllocNode, size: usize, align: usize)
        -> Result<VirtualAddress, ()>
    {
        let alloc_start = align_up(region.start_addr().0, align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr().0 {
            return Err(());
        }

        let excess_size = region.end_addr().0 - alloc_end;
        if excess_size > 0 &&
           excess_size < core::mem::size_of::<AllocNode>()
        {
            return Err(());
        }

        Ok(VirtualAddress(alloc_start))
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(core::mem::align_of::<AllocNode>())
            .expect("Failed to adjust the layout alignment")
            .pad_to_align();
        let size = layout.size().max(core::mem::size_of::<AllocNode>());
        (size, layout.align())
    }

    unsafe fn alloc_memory(&mut self, layout: Layout)
        -> Option<VirtualAddress>
    {
        let (size, align) = Self::size_align(layout);

        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start.0.checked_add(size)
                .expect("Overflow");
            let excess_size = region.end_addr().0 - alloc_end;
            if excess_size > 0 {
                self.add_free_region(VirtualAddress(alloc_end), excess_size);
            }

            Some(alloc_start)
        } else {
            None
        }
    }
}

unsafe impl GlobalAlloc for Locked<Allocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        println!("Trying to allocate: {:#?}", layout);
        let result = ALLOCATOR.lock().alloc_memory(layout)
            .expect("Failed to allocate memory");

        result.0 as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!();
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size())
}

static BOOT_PHYSICAL_MEMORY: BootPhysicalMemory = BootPhysicalMemory {};

#[global_allocator]
static ALLOCATOR: Locked<Allocator> = Locked::new(Allocator::new());

// Linker variables
extern {
    static _end: u32;
}

fn initialize_heap() {
    let heap_start = unsafe { VirtualAddress(&_end as *const u32 as usize) };
    let heap_size = 1 * 1024 * 1024;
    unsafe {
        ALLOCATOR.lock().init(heap_start, heap_size);
    }
}

#[no_mangle]
extern fn kernel_init(multiboot_addr: usize) -> ! {
    serial::initialize();

    let ptr = 0xb8000 as *mut u16;
    unsafe {
        for i in 0..25*80 {
            *ptr.offset(i) = 0x0000;
        }
    }

    initialize_heap();

    let multiboot = unsafe {
        Multiboot::from_addr(&BOOT_PHYSICAL_MEMORY,
                             PhysicalAddress(multiboot_addr))
    };

    // display_multiboot_tags(&multiboot);
    display_memory_map(&multiboot);

    let s = multiboot.find_command_line();
    println!("Command Line: {:?}", s);

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
