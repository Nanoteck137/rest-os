//! Module to handle processes

use crate::arch::x86_64::Regs;

use alloc::string::String;
use alloc::vec::Vec;
use alloc::borrow::ToOwned;

use core::sync::atomic::{ AtomicUsize, Ordering };

static THREAD_STACK: [u8; 4096] = [0; 4096];

static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

fn idle_thread() {
    loop {}
}

fn next_pid() -> usize {
    NEXT_PID.fetch_add(1, Ordering::SeqCst)
}

pub struct Process {
    name: String,
    pid: usize,

    kernel: bool,

    threads: Vec<Thread>,
}

impl Process {
    pub fn create_idle_process() -> Self {
        let stack_ptr = unsafe { crate::allocate_memory(128) };
        let stack = unsafe { core::slice::from_raw_parts(stack_ptr.0 as *const u8, 128) };

        let thread = Thread::create_kernel_thread("Idle Thread".to_owned(), idle_thread as u64, stack);
        let mut threads = Vec::new();

        Self {
            name: "Idle Process".to_owned(),
            pid: 0,

            kernel: true,
            threads,
        }
    }

    pub fn create_kernel_process(name: String, entry: u64) -> Self {
        let thread = Thread::create_kernel_thread("Process thread".to_owned(),
                                                  entry, &THREAD_STACK);

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

pub struct Thread {
    name: String,
    state: ThreadState,
    control_block: ThreadControlBlock,
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct ThreadControlBlock {
    regs: Regs,
    rip: u64,
    rsp: u64,
}

impl Thread {
    pub fn create_kernel_thread(name: String, entry: u64, stack: &'static [u8])
        -> Self
    {
        let control_block = ThreadControlBlock {
            regs: Regs::default(),
            rip: entry,
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
