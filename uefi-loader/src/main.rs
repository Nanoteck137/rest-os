#![feature(asm)]

#![no_std]
#![no_main]

// TODO(patrik):
//   - Go through the code and comment stuff

#[macro_use] extern crate bitflags;
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

struct FrameAlloc {
    start_address: usize,
    num_allocated_frames: usize,
    max_frames: usize,
}

impl FrameAlloc {
    fn new(num_frames: usize) -> Self {
        let start_address = efi::allocate_pages(num_frames)
            .expect("Failed to allocate pages for the Frame Allocator");
        println!("Allocated #{}: {:#x} for the frame allocator",
                 num_frames, start_address);

        Self {
            start_address,
            num_allocated_frames: 0,
            max_frames: num_frames,
        }
    }
}

impl FrameAlloc {
    fn alloc(&mut self) -> usize {
        if self.num_allocated_frames >= self.max_frames {
            panic!("Out of frames");
        }

        let result = self.start_address + self.num_allocated_frames * 4096;
        self.num_allocated_frames += 1;

        result
    }

    fn alloc_zeroed(&mut self) -> usize {
        let result = self.alloc();
        unsafe {
            core::ptr::write_bytes(result as *mut u8, 0, 4096);
        }

        result
    }
}

/// Read the cr3 register
unsafe fn read_cr3() -> u64 {
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3);

    cr3
}

unsafe fn map_page_4k(frame_alloc: &mut FrameAlloc,
                   page_table: u64, vaddr: u64, paddr: u64)
    -> Option<()>
{
    const PAGE_PRESENT: u64 = 1 << 0;
    const PAGE_WRITE: u64 = 1 << 1;
    let page_table_ptr = page_table as *mut u64;

    let p1 = ((vaddr >> 12) & 0x1ff) as usize;
    let p2 = ((vaddr >> 21) & 0x1ff) as usize;
    let p3 = ((vaddr >> 30) & 0x1ff) as usize;
    let p4 = ((vaddr >> 39) & 0x1ff) as usize;

    let mut current_table_ptr = page_table_ptr;

    let index = p4;
    let entry = core::ptr::read(current_table_ptr.add(index));
    if entry & PAGE_PRESENT != PAGE_PRESENT {
        let addr = frame_alloc.alloc_zeroed() as u64;
        let new_entry = addr | PAGE_WRITE | PAGE_PRESENT;
        core::ptr::write(current_table_ptr.add(index), new_entry);

        current_table_ptr = addr as *mut u64;
    } else {
        current_table_ptr = (entry & 0x000ffffffffff000) as *mut u64;
    }

    let index = p3;
    let entry = core::ptr::read(current_table_ptr.add(index));
    if entry & PAGE_PRESENT != PAGE_PRESENT {
        let addr = frame_alloc.alloc_zeroed() as u64;
        let new_entry = addr | PAGE_WRITE | PAGE_PRESENT;
        core::ptr::write(current_table_ptr.add(index), new_entry);

        current_table_ptr = addr as *mut u64;
    } else {
        current_table_ptr = (entry & 0x000ffffffffff000) as *mut u64;
    }

    let index = p2;
    let entry = core::ptr::read(current_table_ptr.add(index));
    if entry & PAGE_PRESENT != PAGE_PRESENT {
        let addr = frame_alloc.alloc_zeroed() as u64;
        let new_entry = addr | PAGE_WRITE | PAGE_PRESENT;
        core::ptr::write(current_table_ptr.add(index), new_entry);

        current_table_ptr = addr as *mut u64;
    } else {
        current_table_ptr = (entry & 0x000ffffffffff000) as *mut u64;
    }

    let index = p1;
    // let entry = core::ptr::read(current_table_ptr.add(index));
    let entry = paddr | PAGE_WRITE | PAGE_PRESENT;
    core::ptr::write(current_table_ptr.add(index), entry);

    Some(())
}

#[no_mangle]
fn efi_main(_image_handle: EfiHandle, table: EfiSystemTablePtr) -> ! {
    unsafe {
        table.register();
    }

    // TODO(patrik): Have a copy of the kernel.elf inside this executable
    // TODO(patrik): Setup the kernel page table
    // TODO(patrik): Load in the kernel
    // TODO(patrik): Load the initrd
    // TODO(patrik): Code cleanup
    // TODO(patrik): Create some kind of structure to pass in to the kernel
    //   - Starting Heap
    //   - Memory map
    //   - ACPI Tables
    //   - Kernel command line, Where from to retrive the command line?
    //     - Read from a file?
    //     - Embed inside the bootloader or kernel executable?
    //   - Initrd
    //   - Early identity map of physical memory
    //   - Framebuffer

    efi::clear_screen()
        .expect("Failed to clear the screen");

    let cr3 = unsafe { read_cr3() };
    println!("CR3: {:#x}", cr3);

    let mut frame_alloc = FrameAlloc::new(512);

    let elf = Elf::parse(&KERNEL_BIN)
        .expect("Failed to parse kernel elf");
    for program_header in elf.program_headers() {
        if program_header.typ() != ProgramHeaderType::Load {
            continue;
        }

        let memory_size = program_header.memory_size();
        let alignment = program_header.alignment();
        assert!(alignment == 0x1000, "We only support an alignment of 4096");

        let page_count = (memory_size + (alignment - 1)) / alignment;
        let page_count = page_count as usize;

        let addr = efi::allocate_pages(page_count)
            .expect("Failed to allocate pages");

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

        for index in (0..memory_size).step_by(4096) {
            let offset = index;
            let vaddr = program_header.vaddr() + offset;
            let paddr = addr as u64 + offset;

            unsafe {
                map_page_4k(&mut frame_alloc, cr3, vaddr, paddr);
            }
        }
    }

    let mut buffer = [0; 4096];

    let (memory_map_size, _, descriptor_size) = efi::memory_map(&mut buffer)
        .expect("Failed to retrive the memory map");

    for offset in (0..memory_map_size).step_by(descriptor_size) {
        let descriptor = efi::MemoryDescriptor::parse(&buffer[offset..])
            .expect("Failed to parse memory descriptor");

        let start = descriptor.start();
        let length = descriptor.length();
        let end = descriptor.end() - 1;

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

        print!("{:?}", descriptor.typ());
        println!();
    }

    // TODO(patrik): Exit boot services here

    type KernelEntry =
        unsafe extern "sysv64" fn(multiboot_structure: u64) -> !;

    let entry: KernelEntry = unsafe { core::mem::transmute(entry_point) };
    unsafe {
        (entry)(0);
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}
