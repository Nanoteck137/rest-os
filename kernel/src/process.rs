//! Module to handle processes

use crate::arch::x86_64::Regs;
use crate::mm;
use crate::mm::PAGE_SIZE;

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

#[derive(Debug)]
pub struct Process {
    name: String,
    pid: usize,

    kernel: bool,

    threads: Vec<Thread>,
}

impl Process {
    pub fn create_idle_process() -> Self {
        let stack = mm::allocate_kernel_vm_zero("Idle Thread Stack".to_owned(),
                                                PAGE_SIZE)
            .expect("Failed to allocate stack for idle thread");
        let stack = unsafe {
            core::slice::from_raw_parts(stack.0 as *const u8,
                                        PAGE_SIZE)
        };

        let thread = Thread::create_kernel_thread("Idle Thread".to_owned(),
                                                  idle_thread, stack);
        let mut threads = Vec::new();
        threads.push(thread);

        Self {
            name: "Idle Process".to_owned(),
            pid: 0,

            kernel: true,
            threads,
        }
    }

    pub fn create_kernel_process(name: String, entry: fn()) -> Self
    {
        let stack =
            if let Some(addr) =
                mm::allocate_kernel_vm_zero(format!("'{}': Stack", name),
                                            PAGE_SIZE)
        {
            addr
        } else {
            panic!("Failed to allocate stack memory for: {}", name);
        };

        let stack = unsafe {
            core::slice::from_raw_parts(stack.0 as *const u8,
                                        PAGE_SIZE)
        };

        let thread =
            Thread::create_kernel_thread(format!("'{}': Main Thread", name),
                                         entry, stack);

        let mut threads = Vec::new();
        threads.push(thread);

        let pid = next_pid();

        Self {
            name,
            pid,

            kernel: true,

            threads
        }
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn pid(&self) -> usize {
        self.pid
    }

    pub fn thread(&self, i: usize) -> Option<&Thread> {
        self.threads.get(i)
    }

    pub fn thread_mut(&mut self, i: usize) -> Option<&mut Thread> {
        self.threads.get_mut(i)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum ThreadState {
    Ready,
    Stopped,
    Running,
    Done,
}

#[derive(Debug)]
pub struct Thread {
    name: String,
    state: ThreadState,
    control_block: ThreadControlBlock,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
pub struct ThreadControlBlock {
    regs: Regs,
    rip: u64,
    rsp: u64,
}

impl Thread {
    pub fn create_kernel_thread(name: String, entry: fn(),
                                   stack: &'static [u8])
        -> Self
    {
        let control_block = ThreadControlBlock {
            regs: Regs::default(),
            rip: entry as u64,
            rsp: stack.as_ptr() as u64 + stack.len() as u64,
        };

        Self {
            name,
            state: ThreadState::Ready,
            control_block,
        }
    }

    pub fn state(&self) -> ThreadState {
        self.state
    }

    pub fn set_state(&mut self, state: ThreadState) {
        self.state = state;
    }

    pub fn control_block(&self) -> ThreadControlBlock {
        self.control_block
    }
}
