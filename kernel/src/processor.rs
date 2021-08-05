//! Module to interface with processor

use crate::mm::{ VirtualAddress, PhysicalAddress, PhysicalMemory };
use crate::mm::frame_alloc::FrameAllocator;
use crate::arch;
use crate::scheduler::Scheduler;

#[macro_export]
macro_rules! core {
    () => {
        $crate::processor::get_local_info()
    }
}

#[repr(C)]
pub struct ProcessorInfo {
    address: VirtualAddress,
    core_id: u32,

    // This cores own scheduler
    scheduler: Scheduler
}

impl ProcessorInfo {
    pub fn core_id(&self) -> u32 {
        self.core_id
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }
}

pub fn get_local_info() -> &'static ProcessorInfo {
    let ptr = unsafe {
        let ptr: u64;
        asm!("mov {}, gs:[0]", out(reg) ptr);

        ptr
    };

    unsafe { &*(ptr as *const ProcessorInfo) }
}

pub fn init<F, P>(frame_allocator: &mut F, physical_memory: &P, core_id: u32)
    where F: FrameAllocator,
          P: PhysicalMemory
{
    // We allocate a frame so we can store all the infomation a processor core
    // needs to have
    let frame = frame_allocator.alloc_frame()
        .expect("Failed to allocate frame for the ProcessorInfo");
    let addr = PhysicalAddress::from(frame);
    let vaddr = physical_memory.translate(addr)
        .expect("No translation for the address");

    // Create the structure for the core infomation
    let processor_info = ProcessorInfo {
        address: vaddr,
        core_id,

        scheduler: Scheduler::new(),
    };

    unsafe {
        // Write that infomation to the frame we allocated
        core::ptr::write(vaddr.0 as *mut ProcessorInfo, processor_info);
        // Set the kernel gs base to that we can access that infomation
        arch::x86_64::write_kernel_gs_base(vaddr.0 as u64);
        // We need to swapgs to have the kernel gs as the current gs
        asm!("swapgs");
    }
}
