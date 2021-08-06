//! Module to schedule processes and threads

use crate::process::{ Process, Thread, ThreadState, ThreadControlBlock };

use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;

static PROCESSES: Mutex<Vec<Arc<Process>>> = Mutex::new(Vec::new());

extern "C" {
    fn switch_to_thread(control_block: &ThreadControlBlock);
}

pub struct Scheduler {
    idle_process: Process
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            idle_process: Process::create_idle_process(),
        }
    }

    pub unsafe fn next(&self) {
        let control_block = {
            let mut lock = PROCESSES.lock();

            let process = Arc::get_mut(lock.get_mut(0).unwrap()).unwrap();
            let thread = process.thread_mut(0).unwrap();

            thread.set_state(ThreadState::Running);

            let control_block = thread.control_block();
            println!("Picking next: {}", process.name());

            control_block
        };

        switch_to_thread(&control_block);
    }

    pub fn add_process(process: Arc<Process>) {
        PROCESSES.lock().push(process);
    }

    pub fn debug_dump_processes() {
        let lock = PROCESSES.lock();

        println!("----------------");

        for process in lock.iter() {
            println!("Process - {}:{}", process.pid(), process.name());
        }

        println!("----------------");
    }
}

global_asm!(r#"
.global switch_to_thread
switch_to_thread:
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
