//! Module to handle processes

use crate::arch::x86_64:: { Regs, PageTable };
use crate::elf::{ Elf, ProgramHeaderType, ProgramHeaderFlags };
use crate::mm;
use crate::mm::{ VirtualAddress, PAGE_SIZE };

use bitflags::bitflags;

use spin::RwLock;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::borrow::ToOwned;
use alloc::sync::Arc;

use core::sync::atomic::{ AtomicUsize, Ordering };

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

fn idle_thread() {
    loop {}
}

fn next_pid() -> usize {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

bitflags! {
    struct TaskFlags: u32 {
        const KERNEL = 0b00000001;
    }
}

#[derive(Copy, Clone, Default, Debug)]
#[repr(C, packed)]
pub struct ControlBlock {
    pub regs:   Regs,
    pub rip:    u64, // 0x78
    pub rsp:    u64, // 0x80
    pub rflags: u64, // 0x88
    pub cr3:    u64, // 0x90
    pub cs:     u64, // 0x98
    pub ss:     u64, // 0xA0
    pub ds:     u64, // 0xA8
    pub es:     u64, // 0xB0
}

bitflags! {
    pub struct MemoryRegionFlags: u32 {
        const READ    = 1 << 0;
        const WRITE   = 1 << 1;
        const EXECUTE = 1 << 2;
    }
}

#[derive(Debug)]
struct MemoryRegion {
    addr: VirtualAddress,
    size: usize,
    flags: MemoryRegionFlags,
}

impl MemoryRegion {
    fn new(addr: VirtualAddress, size: usize,
           flags: MemoryRegionFlags)
        -> Self
    {
        Self {
            addr,
            size,
            flags
        }
    }
}

#[derive(Debug)]
pub struct MemorySpace {
    regions: Vec<MemoryRegion>,
    page_table: PageTable,
}

impl MemorySpace {
    fn new() -> Self {
        Self {
            regions: Vec::new(),
            page_table: mm::create_page_table(),
        }
    }

    pub fn add_region(&mut self, addr: VirtualAddress, size: usize,
                      flags: MemoryRegionFlags)
    {
        // TODO(patrik): Check if we already has a region or if this new
        // region overlaps other regions

        let region = MemoryRegion::new(addr, size, flags);
        self.regions.push(region);
    }

    pub fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    pub fn page_table_mut(&mut self) -> &mut PageTable {
        &mut self.page_table
    }
}

#[derive(Debug)]
pub struct Task {
    name: String,
    flags: TaskFlags,
    pid: usize,

    control_block: ControlBlock,
    // NOTE(patrik): If we are a kernel task then we don't need memory space
    // because a kernel task is always executing inside kernel space
    // TODO(patrik): Task should share memory space with other tasks that are
    // that share the space address space (like child threads)
    memory_space: Option<Arc<RwLock<MemorySpace>>>,
}

impl Task {
    pub fn create_kernel_task(name: String, entry: fn()) -> Self {
        let stack_size = PAGE_SIZE * 2;
        let stack = mm::allocate_kernel_vm_zero(format!("{}: Stack", name),
                                                stack_size)
            .expect("Failed to allocate kernel task stack");

        let flags = TaskFlags::KERNEL;
        let pid = next_pid();

        let mut control_block = ControlBlock::default();
        control_block.rip = entry as u64;
        control_block.rsp = (stack.0 + stack_size) as u64;
        control_block.cr3 = mm::kernel_task_cr3();

        control_block.cs = 0x08;
        control_block.ss = 0x10;
        control_block.ds = 0x10;
        control_block.es = 0x10;

        Self {
            name,
            flags,
            pid,
            control_block,
            memory_space: None,
        }
    }

    pub fn replace_image(&mut self, elf: &Elf) {
        // Reset the control block
        self.control_block = ControlBlock::default();
        self.control_block.cs = 0x30 | 3;
        self.control_block.ss = 0x28 | 3;
        self.control_block.ds = 0x28 | 3;
        self.control_block.es = 0x28 | 3;

        let mut memory_space = MemorySpace::new();

        let old_cr3: u64;

        // TODO(patrik): Should we do this
        unsafe {
            asm!("mov {}, cr3", out(reg) old_cr3);

            let addr = memory_space.page_table().addr().0 as u64;
            asm!("mov cr3, {}", in(reg) addr);
        }

        // Load the program headers
        for program_header in elf.program_headers() {
            if program_header.typ() == ProgramHeaderType::Load {
                println!("Load: {:#x?}", program_header);
                // assert!(program_header.alignment() == 0x1000);

                let mut flags = MemoryRegionFlags::empty();
                if program_header.flags().contains(ProgramHeaderFlags::READ) {
                    flags |= MemoryRegionFlags::READ;
                }

                if program_header.flags().contains(ProgramHeaderFlags::WRITE) {
                    flags |= MemoryRegionFlags::WRITE;
                }

                if program_header.flags()
                        .contains(ProgramHeaderFlags::EXECUTE)
                {
                    flags |= MemoryRegionFlags::EXECUTE;
                }

                let data = elf.program_data(&program_header);
                let size = program_header.memory_size() as usize;
                mm::map_in_userspace(&mut memory_space,
                                     program_header.vaddr(), size, flags)
                    .expect("Failed to map in userspace");

                let source = data.as_ptr();
                let dest = program_header.vaddr().0 as *mut u8;
                let count = size;
                unsafe {
                    core::ptr::copy_nonoverlapping(source, dest, count);
                }
            }
        }

        // TODO(patrik): Change the stack start
        // TODO(patrik): What should the initial stack size be
        let stack_start = VirtualAddress(0x0000700000000000);
        let stack_size = PAGE_SIZE * 4;
        mm::map_in_userspace(&mut memory_space,
                             stack_start, stack_size,
                             MemoryRegionFlags::READ |
                             MemoryRegionFlags::WRITE)
            .expect("Failed to map in stack");

        unsafe {
            core::ptr::write_bytes(stack_start.0 as *mut u8, 0, stack_size);
        }

        // Set the rsp register to the stack we allocated
        self.control_block.rsp = (stack_start.0 + stack_size) as u64;
        // Set the rip register to the elf entry
        self.control_block.rip = elf.entry();
        self.control_block.cr3 = memory_space.page_table().addr().0 as u64;

        self.memory_space = Some(Arc::new(RwLock::new(memory_space)));

        unsafe {
            asm!("mov cr3, {}", in(reg) old_cr3);
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn pid(&self) -> usize {
        self.pid
    }

    pub fn control_block(&self) -> ControlBlock {
        self.control_block
    }

    pub fn add_memory_space_region(&mut self,
                                   vaddr: VirtualAddress, size: usize,
                                   flags: MemoryRegionFlags)
    {
        let memory_space = self.memory_space.as_ref().unwrap();
        let mut lock = memory_space.write();

        lock.add_region(vaddr, size, flags);
    }

    pub fn memory_space(&mut self) -> &Arc<RwLock<MemorySpace>> {
        self.memory_space.as_ref().unwrap()
    }
}

pub fn replace_image_exec(path: String) -> ! {
    let (ptr, size) = crate::read_initrd_file(path)
        .expect("Failed to find file");
    let file = unsafe { core::slice::from_raw_parts(ptr, size) };

    let elf = Elf::parse(&file)
        .expect("Failed to parse file");

    {
        // Switch out the image for the current task
        let task = core!().task();
        let mut lock = task.write();
        lock.replace_image(&elf);
    }

    unsafe {
        // Execute the current task
        core!().scheduler().exec();
    }
}
