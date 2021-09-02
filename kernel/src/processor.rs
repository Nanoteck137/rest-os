//! Module to interface with processor

use crate::mm;
use crate::mm::{ VirtualAddress, PhysicalAddress, PhysicalMemory, PAGE_SIZE };
use crate::mm::FrameAllocator;
use crate::arch;
use crate::arch::ArchInfo;
use crate::arch::x86_64::PageTable;
use crate::scheduler::Scheduler;
// use crate::process::Process;
use crate::process::Task;
use crate::util::{ AutoAtomicRef, AutoAtomicRefGuard };

use alloc::string::String;
use alloc::sync::Arc;
use spin::RwLock;

use core::sync::atomic::{ AtomicUsize, Ordering };

#[macro_export]
macro_rules! core {
    () => {
        $crate::processor::get_local_info()
    }
}

#[repr(C)]
pub struct ProcessorInfo {
    address: usize,
    syscall_stack: usize,
    syscall_saved_stack: usize,

    core_id: u32,

    interrupt_depth: AutoAtomicRef,
    interrupt_disable_ref: AtomicUsize,

    arch: ArchInfo,

    // This cores own scheduler
    scheduler: Scheduler
}

impl ProcessorInfo {
    pub fn core_id(&self) -> u32 {
        self.core_id
    }

    pub fn scheduler(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    pub fn arch(&mut self) -> &mut ArchInfo {
        &mut self.arch
    }

    pub fn page_table(&self) -> PageTable {
        let cr3 = unsafe { arch::x86_64::read_cr3() };

        let page_table =
            unsafe { PageTable::from_table(PhysicalAddress(cr3 as usize)) };

        page_table
    }

    pub fn task(&mut self) -> Arc<RwLock<Task>> {
        self.scheduler.current_task()
    }

    pub unsafe fn enter_interrupt(&self) -> AutoAtomicRefGuard {
        self.interrupt_depth.increment()
    }

    pub unsafe fn enable_interrupts(&self) {
        let value = self.interrupt_disable_ref.fetch_sub(1, Ordering::SeqCst);
        value.checked_sub(1)
            .expect("core!().enable_interrupts(): interrupt disable \
                     reference has underflowed");

        if value == 1 {
            arch::force_enable_interrupts();
        }
    }

    pub unsafe fn disable_interrupts(&self) {
        let value = self.interrupt_disable_ref.fetch_add(1, Ordering::SeqCst);
        value.checked_add(1)
            .expect("core!().enable_interrupts(): interrupt enable reference \
                    has overflowed");

        arch::force_disable_interrupts();
    }

    pub fn without_interrupts<F>(&self, func: F)
        where F: FnOnce()
    {
        unsafe { self.disable_interrupts() };

        func();

        unsafe { self.enable_interrupts() };
    }

    pub fn is_interrupts_enabled(&self) -> bool {
        let flags = unsafe { arch::x86_64::read_flags() };

        flags & 0x200 == 0x200
    }
}

pub fn get_local_info() -> &'static mut ProcessorInfo {
    let ptr = unsafe {
        let ptr: u64;
        asm!("mov {}, gs:[0]", out(reg) ptr);

        ptr
    };

    unsafe { &mut *(ptr as *mut ProcessorInfo) }
}

pub fn init(core_id: u32)
{
    let addr = mm::allocate_kernel_vm(format!("Processor Info: {}", core_id),
                                      core::mem::size_of::<ProcessorInfo>())
        .expect("Failed to allocate memory for Processor Info");

    println!("Returned addr: {:?}", addr);

    let stack_size = PAGE_SIZE * 2;
    let stack_addr =
        mm::allocate_kernel_vm(format!("Core {}: Syscall Stack", core_id),
                               stack_size)
        .expect("Failed to allocate memory for Processor Info");

    // Create the structure for the core infomation
    let processor_info = ProcessorInfo {
        address: addr.0,
        syscall_stack: stack_addr.0 + stack_size,
        syscall_saved_stack: 0,

        core_id,

        interrupt_depth: AutoAtomicRef::new(0),
        // NOTE(patrik): The 'interrupt_disable_ref' is initialized with 1
        // because when booting the interrupts are already disabled to
        // one reference is needed when re-enabling the interrupts after
        // kernel initialization
        interrupt_disable_ref: AtomicUsize::new(1),

        arch: ArchInfo::new(),

        scheduler: Scheduler::new(),
    };

    unsafe {
        // Write that infomation to the frame we allocated
        core::ptr::write(addr.0 as *mut ProcessorInfo, processor_info);
        // Set the kernel gs base to that we can access that infomation
        arch::x86_64::write_kernel_gs_base(addr.0 as u64);

        // We need to swapgs to have the kernel gs as the current gs
        asm!("swapgs");
    }
}
