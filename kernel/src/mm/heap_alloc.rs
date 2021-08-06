//! The kernel heap allocator
//! Design from: https://os.phil-opp.com/allocator-designs/

use super::VirtualAddress;
use crate::util::{ Locked, align_up };
use alloc::alloc::{ Layout, GlobalAlloc };

/// Represents a node inside the free list inside the allocator
/// The allocator is a linked list allocator so we have a list of free nodes
/// and a node is just a size and a ptr to the next entry inside the list and
/// to get the address for the free region we just need to take the address of
/// the node
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

/// This is the main heap allocator so we can use the 'alloc' crate
/// This allocator design is not that efficient but for now this is enough
/// In the future we can implement another allocator design
pub struct Allocator {
    head: AllocNode
}

impl Allocator {
    /// Creates an empty allocator with no memory assigned
    pub const fn new() -> Self {
        Self {
            head: AllocNode::new(0)
        }
    }

    /// Initialize the allocator with some memory
    pub unsafe fn init(&mut self,
                       heap_start: VirtualAddress,
                       heap_size: usize)
    {
        // Add the memory we got from the user and add that as a free region
        self.add_free_region(heap_start, heap_size);
    }

    /// Add a region that we can use to allocate memory from
    unsafe fn add_free_region(&mut self, addr: VirtualAddress, size: usize) {
        // Do some checks to the parameters so they meet the requirements
        assert_eq!(align_up(addr.0, core::mem::align_of::<AllocNode>()),
                   addr.0);
        assert!(size >= core::mem::size_of::<AllocNode>());

        // Create a new node
        let mut node = AllocNode::new(size);
        // Update the linked list nodes
        node.next = self.head.next.take();
        // Create a ptr from the address
        let node_ptr = addr.0 as *mut AllocNode;
        // Write the new node to the ptr
        node_ptr.write(node);
        // Set the next to the new node ptr
        self.head.next = Some(&mut *node_ptr);
    }

    /// Find a free region that meets the requirements
    fn find_region(&mut self, size: usize, align: usize)
        -> Option<(&'static mut AllocNode, VirtualAddress)>
    {
        let mut current = &mut self.head;

        // Loop through the free list to find a good region that meet
        // the requirement
        while let Some(ref mut region) = current.next {
            // Test to see if this region is suitable for the requirements
            if let Ok(alloc_start) =
                Self::alloc_from_region(&region, size, align)
            {
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;

                return ret;
            } else {
                // Go to the next in the list
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    /// Helper function to allocate memory from a region
    fn alloc_from_region(region: &AllocNode, size: usize, align: usize)
        -> Result<VirtualAddress, ()>
    {
        // Align the start to the alignment the user supplied
        let alloc_start = align_up(region.start_addr().0, align);
        // Then we calculate the end of the allocation region
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        // Check if the end address is inside the region
        if alloc_end > region.end_addr().0 {
            return Err(());
        }

        // Calculate the remaining size of the region
        let excess_size = region.end_addr().0 - alloc_end;
        // Check if the remainging size is still suitable for more
        // allocations
        if excess_size > 0 &&
           excess_size < core::mem::size_of::<AllocNode>()
        {
            return Err(());
        }

        Ok(VirtualAddress(alloc_start))
    }

    /// Helper function to make a layout compatible with allocations from
    /// this allocator
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(core::mem::align_of::<AllocNode>())
            .expect("Failed to adjust the layout alignment")
            .pad_to_align();
        let size = layout.size().max(core::mem::size_of::<AllocNode>());
        (size, layout.align())
    }

    /// Find and allocate memory from the allocator
    pub unsafe fn alloc_memory(&mut self, layout: Layout)
        -> Option<VirtualAddress>
    {
        // We need to align the layout so it meets the requirements for
        // this allocator
        let (size, align) = Self::size_align(layout);

        // Try to find a compatible region to allocate memory from
        if let Some((region, alloc_start)) = self.find_region(size, align) {
            // Get the end from the allocation region
            let alloc_end = alloc_start.0.checked_add(size)
                .expect("Overflow");
            // Calculate the remaining size of the region we
            // allocated memory from
            let excess_size = region.end_addr().0 - alloc_end;
            // If the remaining size is greater then 0 then we can add back
            // the remaining bytes to the free list
            if excess_size > 0 {
                self.add_free_region(VirtualAddress(alloc_end), excess_size);
            }

            // Return the start of the region we allocate from
            Some(alloc_start)
        } else {
            // We didn't find a region to allocate from
            None
        }
    }

    /// Free some memory
    pub unsafe fn free_memory(&mut self, addr: VirtualAddress, size: usize) {
        // Add the regoin we got back to the free list
        self.add_free_region(addr, size);
    }
}

/// Implement the GlobalAlloc trait so we can use the Heap Allocator for the
/// alloc crate
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

