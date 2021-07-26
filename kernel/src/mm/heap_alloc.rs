//! The kernel heap allocator
//! Design from: https://os.phil-opp.com/allocator-designs/

use super::VirtualAddress;
use crate::util::{ Locked, align_up };
use alloc::alloc::{ Layout, GlobalAlloc };

struct AllocNode {
    size: usize,
    next: Option<&'static mut AllocNode>
}

impl AllocNode {
    const fn new(size: usize) -> Self {
        Self {
            size,
            next: None
        }
    }

    fn start_addr(&self) -> VirtualAddress {
        VirtualAddress(self as *const Self as usize)
    }

    fn end_addr(&self) -> VirtualAddress {
        self.start_addr() + self.size
    }
}

pub struct Allocator {
    head: AllocNode
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            head: AllocNode::new(0)
        }
    }

    pub unsafe fn init(&mut self,
                       heap_start: VirtualAddress,
                       heap_size: usize)
    {
        self.add_free_region(heap_start, heap_size);
    }

    unsafe fn add_free_region(&mut self, addr: VirtualAddress, size: usize) {
        assert_eq!(align_up(addr.0, core::mem::align_of::<AllocNode>()),
                   addr.0);
        assert!(size >= core::mem::size_of::<AllocNode>());

        let mut node = AllocNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr.0 as *mut AllocNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr);
    }

    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut AllocNode, VirtualAddress)>
    {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) =
                Self::alloc_from_region(&region, size, align)
            {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;

                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    fn alloc_from_region(region: &AllocNode, size: usize, align: usize)
        -> Result<VirtualAddress, ()>
    {
        let alloc_start = align_up(region.start_addr().0, align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr().0 {
            return Err(());
        }

        let excess_size = region.end_addr().0 - alloc_end;
        if excess_size > 0 &&
           excess_size < core::mem::size_of::<AllocNode>()
        {
            return Err(());
        }

        Ok(VirtualAddress(alloc_start))
    }

    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(core::mem::align_of::<AllocNode>())
            .expect("Failed to adjust the layout alignment")
            .pad_to_align();
        let size = layout.size().max(core::mem::size_of::<AllocNode>());
        (size, layout.align())
    }

    unsafe fn alloc_memory(&mut self, layout: Layout)
        -> Option<VirtualAddress>
    {
        let (size, align) = Self::size_align(layout);

        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start.0.checked_add(size)
                .expect("Overflow");
            let excess_size = region.end_addr().0 - alloc_end;
            if excess_size > 0 {
                self.add_free_region(VirtualAddress(alloc_end), excess_size);
            }

            Some(alloc_start)
        } else {
            None
        }
    }

    unsafe fn free_memory(&mut self, addr: VirtualAddress, size: usize) {
        self.add_free_region(addr, size);
    }
}

// Implement the GlobalAlloc trait so we can use the Heap Allocator for the
// alloc crate
unsafe impl GlobalAlloc for Locked<Allocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Try to allocate some memory
        let result = self.lock().alloc_memory(layout)
            .expect("Failed to allocate memory");

        // Convert the result to a pointer and return it
        result.0 as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Get the actual size of the area
        let (size, _) = Allocator::size_align(layout);
        // Convert the pointer to a VirtualAddress
        let addr = VirtualAddress(ptr as usize);
        // Free the memory
        self.lock().free_memory(addr, size);
    }
}

