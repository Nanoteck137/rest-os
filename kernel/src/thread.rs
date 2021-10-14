use crate::process::WeakProcessHandle;
use crate::mm;
use crate::mm::{ VirtualAddress, PAGE_SIZE };

use alloc::sync::{ Arc, Weak };

use spin::RwLock;

pub type ThreadHandle = Arc<RwLock<Thread>>;

#[derive(Copy, Clone, Debug)]
enum ThreadState {
    Runnable,
    Running,
    Stopped,
}

#[derive(Copy, Clone, Default, Debug)]
#[repr(C, packed)]
pub struct ThreadRegisterState {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9:  u64,
    pub r8:  u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,

    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

#[derive(Debug)]
pub struct Thread {
    registers: ThreadRegisterState,
    state: ThreadState,

    stack: VirtualAddress, // If user process this is the stack for the process
                           // If kernel process then this is same as
                           // 'kernel_stack'
    kernel_stack: VirtualAddress,
    kernel_stack_size: usize,

    update: bool,

    id: usize,
    parent: WeakProcessHandle,
}

impl Thread {
    pub fn create(parent: WeakProcessHandle, id: usize, func: fn())
        -> ThreadHandle
    {
        let mut registers = ThreadRegisterState::default();

        let state = ThreadState::Runnable;

        let kernel_stack_size = PAGE_SIZE * 2;
        let kernel_stack =
            mm::allocate_kernel_vm(format!("#{} - Kernel Stack", id),
                                   kernel_stack_size)
                .expect("Failed to allocate kernel stack");

        let kernel_stack_top = kernel_stack.0 + kernel_stack_size;

        // TODO(patrik): Check 'func' so that we are inside kernel space
        registers.rip = func as u64;
        registers.rsp = kernel_stack_top as u64;

        registers.cs = 0x08;
        registers.ss = 0x10;
        registers.rflags = 0x202;

        Arc::new(RwLock::new(Self {
            registers,
            state,

            stack: kernel_stack,
            kernel_stack,
            kernel_stack_size,

            update: true,

            id,
            parent
        }))
    }

    pub fn update(&self) -> bool {
        self.update
    }

    pub fn set_update(&mut self, update: bool) {
        self.update = update;
    }

    pub fn set_registers(&mut self, register_state: ThreadRegisterState) {
        self.registers = register_state;
    }

    pub fn registers(&self) -> ThreadRegisterState {
        self.registers
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn parent(&self) -> &WeakProcessHandle {
        &self.parent
    }

    pub fn kernel_stack(&self) -> VirtualAddress {
        self.kernel_stack
    }

    pub fn kernel_stack_top(&self) -> VirtualAddress {
        VirtualAddress(self.kernel_stack.0 + self.kernel_stack_size)
    }
}
