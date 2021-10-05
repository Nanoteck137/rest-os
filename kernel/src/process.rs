//! Module to handle processes

use crate::arch::x86_64::Regs;
use crate::elf::{ Elf, ProgramHeaderType };
use crate::mm;
use crate::mm::{ VirtualAddress, PAGE_SIZE };

use bitflags::bitflags;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::borrow::ToOwned;

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
    pub regs: Regs,
    pub rip: u64,
    pub rsp: u64,
}

#[derive(Debug)]
pub struct Task {
    name: String,
    flags: TaskFlags,
    pid: usize,

    control_block: ControlBlock,
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

        Self {
            name,
            flags,
            pid,
            control_block,
        }
    }

    pub fn replace_image(&mut self, elf: &Elf) {
        // Reset the control block
        self.control_block = ControlBlock::default();

        // Load the program headers
        for program_header in elf.program_headers() {
            if program_header.typ() == ProgramHeaderType::Load {
                println!("Load: {:#x?}", program_header);
                // assert!(program_header.alignment() == 0x1000);

                let data = elf.program_data(&program_header);
                let size = program_header.memory_size() as usize;
                mm::map_in_userspace(program_header.vaddr(), size)
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
        mm::map_in_userspace(stack_start, stack_size)
            .expect("Failed to map in stack");

        unsafe {
            core::ptr::write_bytes(stack_start.0 as *mut u8, 0, stack_size);
        }

        // Set the rsp register to the stack we allocated
        self.control_block.rsp = (stack_start.0 + stack_size) as u64;
        // Set the rip register to the elf entry
        self.control_block.rip = elf.entry();
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
