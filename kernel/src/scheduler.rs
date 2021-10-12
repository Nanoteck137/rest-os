//! Module to schedule processes and threads

use crate::arch::x86_64::Regs;

use crate::mm;
use crate::mm::{ VirtualAddress, PAGE_SIZE };
//use crate::process::{ Process, Thread, ThreadState, ThreadControlBlock };
use crate::process::{ Task, ControlBlock };
use crate::elf::{ Elf, ProgramHeaderType };

use alloc::collections::LinkedList;
use alloc::sync::Arc;
use alloc::string::String;
use spin::{ Mutex, RwLock };

// static PROCESSES: Mutex<Vec<Arc<RwLock<Process>>>> = Mutex::new(Vec::new());
// TODO(patrik): Replace mutex with a mutex thats disables interrupts?
static TASKS: Mutex<LinkedList<Arc<RwLock<Task>>>> =
    Mutex::new(LinkedList::new());

#[derive(Copy, Clone, Default, Debug)]
#[repr(C, packed)]
pub struct RegisterState {
    pub regs: Regs,
    pub rip:    u64, // 0x78
    pub rsp:    u64, // 0x80
    pub rflags: u64, // 0x88
    pub cr3:    u64, // 0x90
    pub cs:     u64, // 0x98
    pub ss:     u64, // 0xA0
    pub ds:     u64, // 0xA8
    pub es:     u64, // 0xB0
}

extern "C" {
    // fn switch_to_userspace(control_block: &ControlBlock);
    // fn switch_to_kernel(control_block: &ControlBlock);
    fn do_context_switch(register_state: &RegisterState) -> !;
}

pub struct Scheduler {
    current_task: Option<Arc<RwLock<Task>>>,
    ready: bool,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            current_task: None,
            ready: false,
        }
    }

    pub fn set_ready(&mut self) {
        self.ready = true;
    }

    pub unsafe fn tick(&mut self, register_state: RegisterState)
        -> Option<ControlBlock>
    {
        if !self.ready {
            return None;
        }

        let control_block = core!().without_interrupts(|| {
            {
                core!().task().write().update_control_block(register_state);
            }
            let control_block = self.next_when_ready(self.ready);

            control_block
        });

        control_block
    }

    pub unsafe fn force_next(&mut self) -> ! {
        let control_block = self.next_when_ready(true)
            .unwrap();

        // TODO(patrik): This should check if we are switching to
        // kernel or userspace
        if control_block.cs & 3 == 3 {
            asm!("swapgs");
        }

        // TODO(patrik): Check Task::flags to see if we should switch to
        // kernel or userspace
        self.context_switch(control_block);
    }

    pub unsafe fn next(&mut self) -> ! {
        let control_block = self.next_when_ready(self.ready)
            .unwrap();

        // TODO(patrik): This should check if we are switching to
        // kernel or userspace
        if control_block.cs & 3 == 3 {
            asm!("swapgs");
        }

        // TODO(patrik): Check Task::flags to see if we should switch to
        // kernel or userspace
        self.context_switch(control_block);
    }

    unsafe fn next_when_ready(&mut self, ready: bool)
        -> Option<ControlBlock>
    {
        if !ready {
            return None;
        }

        println!("Picking next");

        let control_block = core!().without_interrupts(|| {
            let mut lock = TASKS.lock();

            // Push back the task we currently are executing
            if let Some(task) = self.current_task.take() {
                // TODO(patrik): Check if the task is runnable
                lock.push_back(task);
            }

            let task = lock.pop_front()
                .expect("Failed to pop_front");

            self.current_task = Some(task.clone());

            let task = task.read();

            // TODO(patrik): Set task state to running

            let control_block = task.control_block();
            println!("Picking next: {}", task.name());

            control_block
        });

        println!("Control Block: {:#x?}", control_block);

        Some(control_block)
    }

    pub unsafe fn exec(&mut self) -> ! {
        let control_block = core!().without_interrupts(|| {
            let task = self.current_task();
            let control_block = task.read().control_block();

            control_block
        });

        if control_block.cs & 3 == 3 {
            asm!("swapgs");
        }

        self.context_switch(control_block);
    }

    pub unsafe fn context_switch(&mut self, control_block: ControlBlock) -> ! {
        core!().arch().set_kernel_stack(control_block.kernel_stack);

        let mut register_state = RegisterState::default();
        register_state.regs = control_block.regs;
        register_state.rip = control_block.rip;
        register_state.rsp = control_block.stack;
        register_state.rflags = control_block.rflags;
        register_state.cr3 = control_block.cr3;
        register_state.cs = control_block.cs;
        register_state.ss = control_block.ss;
        register_state.ds = control_block.ds;
        register_state.es = control_block.es;

        do_context_switch(&register_state);
    }

    pub fn current_task(&mut self) -> Arc<RwLock<Task>> {
        // TODO(patrik): Remove 'unwrap'
        self.current_task.as_ref().unwrap().clone()
    }

    pub fn add_task(task: Task) {
        TASKS.lock().push_back(Arc::new(RwLock::new(task)));
    }

    pub fn debug_dump_tasks() {
        let lock = TASKS.lock();

        println!("----------------");

        for task in lock.iter() {
            let task = task.read();
            println!("Task - {}:{}", task.pid(), task.name());
        }

        println!("----------------");
    }
}

global_asm!(r#"
.global do_context_switch
do_context_switch:
    cli

    mov rax, QWORD PTR [rdi + 0xA8]
    mov ds, ax
    mov rax, QWORD PTR [rdi + 0xB0]
    mov es, ax

    // Setup the iretq frame
    push QWORD PTR [rdi + 0xA0] // Stack segment
    push QWORD PTR [rdi + 0x80] // RSP
    push QWORD PTR [rdi + 0x88] // RFLAGS
    push QWORD PTR [rdi + 0x98] // Code segment
    push QWORD PTR [rdi + 0x78] // RIP

    // Setup the cr3 register
    mov rax, QWORD PTR [rdi + 0x90]
    mov cr3, rax

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

    iretq

// rdi - Control Block
switch_to_userspace:
    mov ax, 0x28 | 3
    mov ds, ax
    mov es, ax

    // Setup the iretq frame
    push 0x28 | 3
    // RSP
    push QWORD PTR [rdi + 0x80]
    push QWORD PTR 0x202
    push 0x30 | 3
    // RIP
    push QWORD PTR [rdi + 0x78]

    // Setup the cr3 register
    mov rax, QWORD PTR [rdi + 0x88]
    mov cr3, rax

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

.global switch_to_kernel
switch_to_kernel:
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
