//! Module to schedule processes and threads

use crate::mm;
use crate::process::{ Process, Thread, ThreadState, ThreadControlBlock };
use crate::elf::{ Elf, ProgramHeaderType };

use alloc::vec::Vec;
use alloc::sync::Arc;
use alloc::string::String;
use spin::{ Mutex, RwLock };

static PROCESSES: Mutex<Vec<Arc<RwLock<Process>>>> = Mutex::new(Vec::new());

extern "C" {
    fn switch_to_userspace(control_block: &ThreadControlBlock);
    fn switch_to_kernel_thread(control_block: &ThreadControlBlock);
}

pub struct Scheduler {
    idle_process: Process,
    current_pid: usize,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            idle_process: Process::create_idle_process(),
            current_pid: 0,
        }
    }

    pub unsafe fn next(&mut self) {
        let (control_block, pid) = {
            let lock = PROCESSES.lock();

            let mut process = lock.get(0).unwrap().write();
            let thread = process.thread_mut(0).unwrap();

            thread.set_state(ThreadState::Running);

            let control_block = thread.control_block();
            println!("Picking next: {}", process.name());

            (control_block, process.pid())
        };

        self.current_pid = pid;
        switch_to_kernel_thread(&control_block);
    }

    fn replace_process_image(&mut self, elf: &Elf) -> ! {
        let process = self.current_process();
        let mut lock = process.write();
        let main_thread = lock.thread_mut(0)
            .expect("Failed to retrive main thread");

        // TODO(patrik): If we replace a kernel thread then we need to
        // free the stack
        main_thread.reset();
        main_thread.control_block.rip = elf.entry();

        for program_header in elf.program_headers() {
            if program_header.typ() == ProgramHeaderType::Load {
                println!("Load: {:#x?}", program_header);
                assert!(program_header.alignment() == 0x1000);

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

        core::mem::drop(lock);
        core::mem::drop(process);

        unsafe {
            core!().scheduler().exec();
        }
    }

    unsafe fn exec(&mut self) -> ! {
        let control_block = {
            let process = self.current_process();
            let result = process.read().thread(0).unwrap().control_block();

            result
        };

        println!("Switching to userspace");
        switch_to_userspace(&control_block);

        panic!("Failed to switch to userspace");
    }

    pub fn current_process(&mut self) -> Arc<RwLock<Process>> {
        let lock = PROCESSES.lock();

        for process in lock.iter() {
            if process.read().pid() == self.current_pid {
                return process.clone();
            }
        }

        panic!("No process with pid: {}", self.current_pid);
    }

    pub fn add_process(process: Process) {
        PROCESSES.lock().push(Arc::new(RwLock::new(process)));
    }

    pub fn debug_dump_processes() {
        let lock = PROCESSES.lock();

        println!("----------------");

        for process in lock.iter() {
            let process = process.read();
            println!("Process - {}:{}", process.pid(), process.name());
        }

        println!("----------------");
    }
}

pub fn replace_process_image(path: String) {
    let file = crate::read_initrd_file(path)
        .expect("Failed to find file");
    let elf = Elf::parse(&file)
        .expect("Failed to parse file");

    core!().scheduler().replace_process_image(&elf);
}

global_asm!(r#"
.global switch_to_userspace
// rdi - Control Block
switch_to_userspace:
    mov ax, 0x28 | 3
    mov ds, ax
    mov es, ax

    // Setup the iretq frame
    push 0x28 | 3
    // RSP
    push QWORD PTR [rdi + 0x80]
    push QWORD PTR 0x200
    push 0x20 | 3
    // RIP
    push QWORD PTR [rdi + 0x78]

    mov r15, QWORD PTR [rdi + 0x00]
    mov r14, QWORD PTR [rdi + 0x08]
    mov r13, QWORD PTR [rdi + 0x10]
    mov r12, QWORD PTR [rdi + 0x18]
    mov r11, QWORD PTR [rdi + 0x20]
    mov r10, QWORD PTR [rdi + 0x28]
    mov r9,  QWORD PTR [rdi + 0x30]
    mov r8,  QWORD PTR [rdi + 0x38]
    mov rbp, QWORD PTR [rdi + 0x40]
    // mov rdi, QWORD PTR [rdi + 0x48]
    mov rsi, QWORD PTR [rdi + 0x50]
    mov rdx, QWORD PTR [rdi + 0x58]
    mov rcx, QWORD PTR [rdi + 0x60]
    mov rbx, QWORD PTR [rdi + 0x68]
    mov rax, QWORD PTR [rdi + 0x70]

    // Now we can set push the value RDI needs
    push QWORD PTR [rdi + 0x48]
    // Pop the value to set RDI
    pop rdi

    // Swap the gs to the user gs is used insteed of the kernel gs
    swapgs

    iretq

.global switch_to_kernel_thread
switch_to_kernel_thread:
    mov rsp, QWORD PTR [rdi + 0x80]

    mov r15, QWORD PTR [rdi + 0x00]
    mov r14, QWORD PTR [rdi + 0x08]
    mov r13, QWORD PTR [rdi + 0x10]
    mov r12, QWORD PTR [rdi + 0x18]
    mov r11, QWORD PTR [rdi + 0x20]
    mov r10, QWORD PTR [rdi + 0x28]
    mov r9,  QWORD PTR [rdi + 0x30]
    mov r8,  QWORD PTR [rdi + 0x38]
    mov rbp, QWORD PTR [rdi + 0x40]
    // mov rdi, QWORD PTR [rdi + 0x48]
    mov rsi, QWORD PTR [rdi + 0x50]
    mov rdx, QWORD PTR [rdi + 0x58]
    mov rcx, QWORD PTR [rdi + 0x60]
    mov rbx, QWORD PTR [rdi + 0x68]
    mov rax, QWORD PTR [rdi + 0x70]

    // Push the rip
    push QWORD PTR [rdi + 0x78]

    // Now we can set push the value RDI needs
    push QWORD PTR [rdi + 0x48]
    // Pop the value to set RDI
    pop rdi

    ret
"#);
