use crate::mm;
use crate::mm::{ PAGE_SIZE, VirtualAddress };
use crate::mm::{ MemorySpace, MemoryRegionFlags };
use crate::thread::{ Thread, ThreadHandle, ThreadRegisterState };
use crate::elf::{ Elf, ProgramHeaderType, ProgramHeaderFlags };

use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::{ Arc, Weak };

use spin::RwLock;

pub type ProcessHandle = Arc<RwLock<Process>>;
pub type WeakProcessHandle = Weak<RwLock<Process>>;

bitflags! {
    struct ProcessFlags: u32 {
        const KERNEL = 1 << 0;
    }
}

#[derive(Debug)]
pub struct Process {
    name: String,
    flags: ProcessFlags,

    memory_space: Option<MemorySpace>,

    threads: Vec<ThreadHandle>
}

impl Process {
    pub fn create_idle(core_id: usize, idle_thread_func: fn())
        -> ProcessHandle
    {
        let flags = ProcessFlags::KERNEL;
        let threads = Vec::new();

        let name = format!("Idle Process: #{}", core_id);

        let result = Arc::new(RwLock::new(Self {
            name,
            flags,
            memory_space: None,
            threads
        }));

        let main_thread_id = 0;
        let main_thread = Thread::create(Arc::downgrade(&result),
                                         main_thread_id,
                                         idle_thread_func);

        {
            result.write().add_thread(main_thread);
        }

        result
    }

    pub fn create_kernel(name: String, main_thread_func: fn())
        -> ProcessHandle
    {
        let flags = ProcessFlags::KERNEL;
        let threads = Vec::new();

        let result = Arc::new(RwLock::new(Self {
            name,
            flags,
            memory_space: None,
            threads
        }));

        let main_thread_id = 0;
        let main_thread = Thread::create(Arc::downgrade(&result),
                                         main_thread_id,
                                         main_thread_func);

        {
            result.write().add_thread(main_thread);
        }

        result
    }

    fn replace_image(&mut self, elf: &Elf) {
        verify_interrupts_disabled!();

        // NOTE(patrik):
        // We need to reset all the threads this process has but other
        // cores chould be executing them as we speek so how should we handle
        // that senerio

        // TODO(patrik): This code assumes we only have one core
        // we need to implement APIC support so we can figure out a
        // way to make this work

        let current_thread = core!().thread();
        let mut current_thread_lock = current_thread.write();

        let tid = current_thread_lock.id();

        println!("Changing image for tid #{}", tid);

        let mut new_register_state = ThreadRegisterState::default();
        new_register_state.rflags = 0x202;
        new_register_state.cs = 0x30 | 3;
        new_register_state.ss = 0x28 | 3;

        let mut memory_space = MemorySpace::new();

        // NOTE(patrik): Switch to the new page table so we can copy in the
        // program data
        let old_cr3: u64;

        // TODO(patrik): Should we do this
        unsafe {
            asm!("mov {}, cr3", out(reg) old_cr3);

            let addr = memory_space.page_table().addr().0 as u64;
            asm!("mov cr3, {}", in(reg) addr);
        }

        for program_header in elf.program_headers() {
            if program_header.typ() == ProgramHeaderType::Load {
                println!("Load: {:#x?}", program_header);

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

                let size = program_header.memory_size() as usize;
                let vaddr = VirtualAddress(program_header.vaddr() as usize);
                mm::map_in_userspace(&mut memory_space,
                                     vaddr, size, flags)
                    .expect("Failed to map in userspace");

                let data = elf.program_data(&program_header);

                let source = data.as_ptr();
                let dest = vaddr.0 as *mut u8;
                let count = size;
                // TODO(patrik): How do we copy over the data
                unsafe {
                    // core::ptr::copy_nonoverlapping(source, dest, count);
                }
            }
        }

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

        let user_stack_top = stack_start.0 + stack_size;
        new_register_state.rsp = user_stack_top as u64;
        new_register_state.rip = elf.entry();

        self.memory_space = Some(memory_space);
        self.flags.remove(ProcessFlags::KERNEL);

        unsafe {
            asm!("mov cr3, {}", in(reg) old_cr3);
        }

        current_thread_lock.set_registers(new_register_state);
        current_thread_lock.set_update(false);
    }

    fn add_thread(&mut self, thread: ThreadHandle) {
        self.threads.push(thread)
    }

    pub fn kernel(&self) -> bool {
        self.flags.contains(ProcessFlags::KERNEL)
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn threads(&self) -> &Vec<ThreadHandle> {
        &self.threads
    }

    pub fn main_thread(&self) -> &ThreadHandle {
        &self.threads.get(0)
            .expect("No threads?")
    }

    pub fn memory_space(&self) -> Option<&MemorySpace> {
        self.memory_space.as_ref()
    }

    pub fn memory_space_mut(&mut self) -> Option<&mut MemorySpace> {
        self.memory_space.as_mut()
    }
}

pub unsafe fn replace_image_exec(path: String) {
    let (ptr, size) = crate::read_initrd_file(path)
        .expect("Failed to find file");
    let file = core::slice::from_raw_parts(ptr, size);

    let elf = Elf::parse(&file)
        .expect("Failed to parse file");

    core!().without_interrupts(|| {
        // Switch out the image for the current task
        let process = core!().process();
        let mut process_lock = process.write();
        process_lock.replace_image(&elf);
    });
}
