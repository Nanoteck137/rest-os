//! This is the UEFI loader for the Rest-OS kernel
//!
//! The kernel itself is embedded inside this executable and then later parsed
//! with the [`elf`] crate. Then we retrive the memory map and ACPI table
//! address from EFI with the [`efi`] module.
#![feature(asm)]

#![allow(rustdoc::private_intra_doc_links)]

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

/// ConsoleWriter is responsible to print strings to the EFI Stdout
struct ConsoleWriter {}

impl ConsoleWriter {
    /// Prints `s` to the EFI Stdout
    ///
    /// # Arguments
    ///
    /// * `s` - The string to print
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

/// Prints to the EFI Stdout
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::_print_fmt(format_args!($($arg)*));
    }}
}

/// Prints to the EFI Stdout, with a newline
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)))
}

/// The ConsoleWriter used when printing with [`print`] and [`println`]
static mut WRITER: ConsoleWriter = ConsoleWriter {};

/// Helper function for the [`print`] macro, used for writing `args`
/// to the WRITER
pub fn _print_fmt(args: core::fmt::Arguments) {
    use core::fmt::Write;

    unsafe {
        let _ = WRITER.write_fmt(args);
    }
}

/// The included kernel executable
static KERNEL_EXECUTABLE: &'static [u8] =
    include_bytes!("../../target/kernel.elf");

/// Simple frame allocator, used by the page mapping code to allocate pages
/// for the page table
struct FrameAlloc {
    /// The start address of the allocated region from [`efi::allocate_pages`]
    start_address: usize,
    /// The number of frames the user has allocated, used to offset the
    /// `start_address` to find a new address for new allocations
    num_allocated_frames: usize,
    /// The max number of frames the allocator can allocate
    max_frames: usize,
}

impl FrameAlloc {
    /// Creates a new frame allocator
    ///
    /// # Arguments
    ///
    /// * `num_frames` - The number of frames to allocate from EFI
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

    /// Allocate a frame
    fn alloc(&mut self) -> usize {
        if self.num_allocated_frames >= self.max_frames {
            // TODO(patrik): Should we panic here?
            panic!("Out of frames");
        }

        let result = self.start_address + self.num_allocated_frames * 4096;
        self.num_allocated_frames += 1;

        result
    }

    /// Allocate a frame and zero it out
    fn alloc_zeroed(&mut self) -> usize {
        let result = self.alloc();
        unsafe {
            core::ptr::write_bytes(result as *mut u8, 0, 4096);
        }

        result
    }
}

/// Returns the cr3 register for this CPU core
unsafe fn read_cr3() -> u64 {
    let cr3: u64;
    asm!("mov {}, cr3", out(reg) cr3);

    cr3
}

/// Maps a 4K aligned physical address to a 4K aligned virtual address
/// inside a page table
///
/// # Arguments
///
/// * `frame_alloc` - The frame allocator the mapping code can use to allocate
///    new frames for the page table
/// * `page_table` - The address of the page table the mapping code use to map
///    the physical address to the virtual address
/// * `vaddr` - The virtual address
/// * `paddr` - The physical address
///
/// # Returns
/// * `Some(())` - When the mapping was successful
/// * `None` - When the mapping was unsuccessful
unsafe fn map_page_4k(frame_alloc: &mut FrameAlloc,
                      page_table: u64, vaddr: u64, paddr: u64)
    -> Option<()>
{
    const PAGE_PRESENT: u64 = 1 << 0;
    const PAGE_WRITE: u64 = 1 << 1;
    let page_table_ptr = page_table as *mut u64;

    // Get the indicies for each table
    let p1 = ((vaddr >> 12) & 0x1ff) as usize;
    let p2 = ((vaddr >> 21) & 0x1ff) as usize;
    let p3 = ((vaddr >> 30) & 0x1ff) as usize;
    let p4 = ((vaddr >> 39) & 0x1ff) as usize;

    // The current page table we are working on, this starts as the page table
    // but when we start to walk down the page table we update this with the
    // new table
    let mut current_table_ptr = page_table_ptr;

    // The indicies we want to walk down on
    let indicies = [p4, p3, p2];

    // Loop through all the indicies
    for index in indicies {
        // Read the entry for this index
        let entry = core::ptr::read(current_table_ptr.add(index));
        // Check if the entry is not present
        if entry & PAGE_PRESENT != PAGE_PRESENT {
            // If the entry is not present then we need to allocate a new
            // table and update the entry with the address and flags

            // Allocate the new table (filled with zeroes)
            let addr = frame_alloc.alloc_zeroed() as u64;
            // Construct the new entry with the address we got from the `alloc_zeroed`
            // and or together the flags we want
            let new_entry = addr | PAGE_WRITE | PAGE_PRESENT;
            // Write the new entry to the table at the index
            core::ptr::write(current_table_ptr.add(index), new_entry);

            // Update the current table ptr with the new address
            current_table_ptr = addr as *mut u64;
        } else {
            // If the entry was present then just update the current table ptr
            // with the entry address
            current_table_ptr = (entry & 0x000ffffffffff000) as *mut u64;
        }
    }

    // Now that we have walked down the page table we should be at the last
    // table we need, and the current table ptr should point to the p1 table

    // Write the paddr and flags to the p1 table at the index
    let index = p1;
    let entry = paddr | PAGE_WRITE | PAGE_PRESENT;
    core::ptr::write(current_table_ptr.add(index), entry);

    Some(())
}

/// The main entry point for a EFI application
///
/// # Arguments
///
/// * `image_handle` - The firmware allocated handle for the UEFI image
/// * `table` - A pointer to the EFI System Table
#[no_mangle]
fn efi_main(image_handle: EfiHandle, table: EfiSystemTablePtr) -> ! {
    unsafe {
        table.register();
    }

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

    // Clear the screen
    efi::clear_screen()
        .expect("Failed to clear the screen");

    // Get the cr3
    let cr3 = unsafe { read_cr3() };
    // Mask of the bottom 12-bits
    let page_table = cr3 & !0xfff;
    println!("Current page table address: {:#x}", page_table);

    // Create the frame allocator with a size of 512 frames
    let mut frame_alloc = FrameAlloc::new(512);

    // Parse the kernel executable
    let elf = Elf::parse(&KERNEL_EXECUTABLE)
        .expect("Failed to parse kernel executable");
    // Loop through all the program headers
    for program_header in elf.program_headers() {
        // If the program header is not the type of `ProgramHeaderType::Load`
        // then just continue to the next one
        if program_header.typ() != ProgramHeaderType::Load {
            continue;
        }

        // Get the size in memory this program header takes up
        let memory_size = program_header.memory_size();
        // Get the alignment
        let alignment = program_header.alignment();
        // NOTE(patrik): We only support a alignment of 0x1000 or 4096 because
        // it's easier to map the pages inside the page table. but in the
        // future we could supoprt other alignment but for now we know the
        // alignment is always gonna be 4096 for this kernel
        assert!(alignment == 0x1000, "We only support an alignment of 4096");

        // Calculate the number of pages
        let page_count = (memory_size + (alignment - 1)) / alignment;
        let page_count = page_count as usize;

        // Allocate the necessary pages for the program header
        let addr = efi::allocate_pages(page_count)
            .expect("Failed to allocate pages");

        // Create a pointer from the address we got from `efi::allocate_pages`
        let ptr = addr as *mut u8;

        unsafe {
            // Zero out the allocated region
            core::ptr::write_bytes(ptr, 0, page_count * 0x1000);
        }

        // Get access to the program header data so we can copy over them to
        // the new allocated region
        let data = elf.program_data(&program_header);
        let data_size = program_header.file_size() as usize;

        unsafe {
            // Copy the bytes from the program header to the allocated region
            core::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data_size);
        }

        // Loop through all the pages and map them in at the correct
        // virtual address
        for index in (0..memory_size).step_by(4096) {
            let offset = index;
            // The virtual address we should map the page
            let vaddr = program_header.vaddr() + offset;
            // The physical address of the page
            let paddr = addr as u64 + offset;

            unsafe {
                // Map in the page
                map_page_4k(&mut frame_alloc, cr3, vaddr, paddr);
            }
        }
    }

    // Create a buffer for the efi memory map
    let mut buffer = [0; 4096];

    // Get the memory map
    let (memory_map_size, map_key, descriptor_size) =
        efi::memory_map(&mut buffer)
            .expect("Failed to retrive the memory map");

    // NOTE(patirk): This will exit boot services, but we don't want to do it
    // yet because we lose the ability to print stuff

    /*
    let (memory_map_size, descriptor_size)  = loop {
        let (memory_map_size, map_key, descriptor_size) =
            efi::memory_map(&mut buffer)
                .expect("Failed to retrive the memory map");

        match efi::exit_boot_services(image_handle, map_key) {
            Ok(()) => break (memory_map_size, descriptor_size),
            Err(status) => continue
        }
    };
    */

    // Loop through the memory map and print out the infomation
    for offset in (0..memory_map_size).step_by(descriptor_size) {
        // Parse the descriptor
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

    // Create a type for the kernel entry point
    type KernelEntry =
        unsafe extern "sysv64" fn(multiboot_structure: u64) -> !;

    // Get the address for the entry point inside the kernel executable
    let entry_point = elf.entry();
    // Transmute the entry point to a `KernelEntry` so that we can call into
    // the kernel
    let entry: KernelEntry = unsafe { core::mem::transmute(entry_point) };

    // Call into the kernel
    unsafe {
        (entry)(0);
    }
}

/// The panic handler for this application
///
/// # Arguments
///
/// - `info` - The panic info we retrive from a panic so we can print some
///    useful infomation about the panic
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}
