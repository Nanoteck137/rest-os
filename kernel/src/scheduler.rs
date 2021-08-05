//! Module to schedule processes and threads

use crate::process::{ Process, Thread };

use alloc::vec::Vec;
use alloc::sync::Arc;
use spin::Mutex;

static PROCESSES: Mutex<Vec<Arc<Process>>> = Mutex::new(Vec::new());

pub struct Scheduler {

}

impl Scheduler {
    pub fn new() -> Self {
        Self {
        }
    }

    pub unsafe fn next(&self) {
        let lock = PROCESSES.lock();

        let process = lock.get(0).unwrap();
        let thread = process.thread(0).unwrap();

        let thread_ptr = thread as *const Thread;

        println!("Picking next: {}", process.name());

        core::mem::drop(lock);

        (*thread_ptr).switch_to();
    }

    pub fn add_process(process: Arc<Process>) {
        PROCESSES.lock().push(process);
    }

    pub fn debug_dump_processes() {
        let lock = PROCESSES.lock();

        println!("----------------");

        for process in lock.iter() {
            println!("Process: {}", process.name());
        }

        println!("----------------");
    }
}
