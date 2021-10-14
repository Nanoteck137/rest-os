use crate::mm;
use crate::process::{ Process, ProcessHandle };
use crate::thread::{ ThreadHandle, ThreadRegisterState };

use alloc::collections::LinkedList;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::{ Mutex, RwLock };

static PROCESSES: Mutex<Vec<ProcessHandle>> = Mutex::new(Vec::new());
static THREAD_QUEUE: Mutex<LinkedList<ThreadHandle>> =
    Mutex::new(LinkedList::new());

extern "C" {
    fn switch_thread(register_state: &ThreadRegisterState,
                     page_table_addr: usize) -> !;
}

fn idle_thread() {
    loop {
        println!("Idle Thread");
    }
}

pub struct Scheduler {
    idle_process: ProcessHandle,
    ready: bool,

    current_thread: Option<ThreadHandle>,
}

impl Scheduler {
    pub fn new(core_id: usize) -> Self {
        let idle_process = Process::create_idle(core_id, idle_thread);

        Self {
            idle_process,
            ready: false,
            current_thread: None,
        }
    }

    pub fn set_ready(&mut self) {
        self.ready = true;
    }

    pub unsafe fn start(&mut self) -> ! {
        // TODO(patrik): Context switch

        // let new_thread = self.schedule();

        assert!(self.current_thread.is_none(),
                "Scheduler: current thread should be none");

        let new_thread = {
            let mut thread_queue_lock = THREAD_QUEUE.lock();
            thread_queue_lock.pop_front()
                .expect("No init process created?")
        };

        let register_state = new_thread.read().registers();
        let page_table_addr = mm::kernel_task_cr3() as usize;

        self.current_thread = Some(new_thread);

        switch_thread(&register_state, page_table_addr);
    }

    pub fn schedule(&mut self, register_state: ThreadRegisterState)
        -> Option<(ThreadHandle, u64)>
    {
        if !self.ready {
            return None;
        }

        let mut thread_queue_lock = THREAD_QUEUE.lock();

        if let Some(thread) = self.current_thread.take() {
            // TODO(patrik): Set thread state
            {
                let mut thread_lock = thread.write();

                if thread_lock.update() {
                    thread_lock.set_registers(register_state);
                } else {
                    thread_lock.set_update(true);
                }
            }
            thread_queue_lock.push_back(thread);
        }

        let new_thread = thread_queue_lock.pop_front()
            .expect("Failed to pop_front");

        let parent = new_thread.read().parent().upgrade()
            .expect("Thread no parent?");
        let parent_lock = parent.read();

        println!("Picking new thread from process: {}", parent_lock.name());

        // TODO(patrik): Set thread state

        let cr3 = {
            if let Some(memory_space) = parent_lock.memory_space() {
                memory_space.page_table().addr().0 as u64
            } else if parent_lock.kernel() {
                mm::kernel_task_cr3()
            } else {
                panic!("Can't find cr3 for thread");
            }
        };

        self.current_thread = Some(new_thread.clone());

        Some((new_thread, cr3))
    }

    pub unsafe fn exec(&self) -> ! {
        let (registers, cr3) = {
            let thread = core!().thread();
            let thread_lock = thread.read();

            let parent = core!().process();
            let parent_lock = parent.read();

            let registers = thread_lock.registers();
            let cr3 = {
                if let Some(memory_space) = parent_lock.memory_space() {
                    memory_space.page_table().addr().0
                } else if parent_lock.kernel() {
                    mm::kernel_task_cr3() as usize
                } else {
                    panic!("Can't find cr3 for thread");
                }
            };

            (registers, cr3)
        };

        switch_thread(&registers, cr3);
    }

    pub fn add_process(process: ProcessHandle) {
        let mut process_list_lock = PROCESSES.lock();
        let mut thread_queue_lock = THREAD_QUEUE.lock();

        {
            for thread in process.read().threads().iter() {
                thread_queue_lock.push_back(thread.clone());
            }
        }

        process_list_lock.push(process);
    }

    pub fn debug_dump() {
        let process_list_lock = PROCESSES.lock();
        let thread_queue_lock = THREAD_QUEUE.lock();

        println!("-------------- PROCESSES --------------");
        for process in process_list_lock.iter() {
            println!("  - {}", process.read().name());
        }
        println!("---------------------------------------");

        println!("-------------- THREAD QUEUE --------------");
        for thread in thread_queue_lock.iter() {
            let thread_lock = thread.read();
            let parent = thread_lock.parent().upgrade()
                .expect("Thread without parent");
            let parent_lock = parent.read();
            println!("  - #{} '{}'", thread_lock.id(), parent_lock.name());

            // println!("Thread: {:#x?}", thread);
        }
        println!("------------------------------------------");
    }

    pub fn current_thread(&self) -> ThreadHandle {
        // TODO(patrik): Remove expect
        self.current_thread.as_ref()
            .expect("No thread assigned")
                .clone()
    }
}

global_asm!(r#"
switch_thread:
    mov rsp, rdi
    mov rax, rsi

    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx

    mov cr3, rax

    pop rax

    iretq
"#);
