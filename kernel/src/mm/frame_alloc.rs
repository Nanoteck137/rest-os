//! This is the page frame allocator used by the kernel (page tables)

use super::{ PAGE_SIZE, Frame, PhysicalAddress };
use core::convert::TryFrom;

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame>;
    fn free_frame(&mut self, frame: Frame);
}

// Simple frame allocator for booting the kernel
// This uses memory after the kernel heap to allocate frames
// and those frames can't be freed because this only needs to
// allocate frames then we switch to a more advanced frame allocator
pub struct BootFrameAllocator {
    start: PhysicalAddress,
    end: PhysicalAddress,
}

impl BootFrameAllocator {
    pub fn new(start: PhysicalAddress, end: PhysicalAddress) -> Self {
        assert!(start.0 % PAGE_SIZE == 0,
                "'start' needs to be aligned to PAGE_SIZE");
        assert!(end.0 > start.0,
                "'end' is behind 'start'");
        assert!(end.0 % PAGE_SIZE == 0,
                "'end' needs to be aligned to PAGE_SIZE");

        Self {
            start,
            end
        }
    }
}

impl FrameAllocator for BootFrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        let start = self.start;
        let result = start;

        if result.0 >= self.end.0 {
            return None;
        }

        self.start.0 += PAGE_SIZE;

        Frame::try_from(result).ok()
    }

    fn free_frame(&mut self, _frame: Frame) {
        panic!("Can't free frames using the BootFrameAllocator");
    }
}
