use super::{ PhysicalAddress, VirtualAddress };
use super::{ PHYSICAL_MEMORY_START, PHYSICAL_MEMORY_END };
use super::{ KERNEL_TEXT_START, KERNEL_TEXT_END };

pub trait PhysicalMemory
{
    // Translates a physical address to a virtual address
    fn translate(&self, paddr: PhysicalAddress) -> Option<VirtualAddress>;

    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T;

    // Read from physical memory at paddr (can be unalinged)
    unsafe fn read_unaligned<T>(&self, paddr: PhysicalAddress) -> T;

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T);

    // Slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T];

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T];
}


pub struct BootPhysicalMemory;

impl PhysicalMemory for BootPhysicalMemory {
    // Translates a physical address to a virtual address
    fn translate(&self, paddr: PhysicalAddress) -> Option<VirtualAddress> {
        // TODO(patrik): Add some checks to that the physical address is
        // inside the bounds of the boot physical memory range
        let new_addr = paddr.0 + KERNEL_TEXT_START.0;

        Some(VirtualAddress(new_addr))
    }

    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            KERNEL_TEXT_START.0;
        assert!(end <= KERNEL_TEXT_END.0,
                "Reading address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START.0;
        core::ptr::read_volatile(new_addr as *const T)
    }

    // Read from physical memory at paddr (can be unalinged)
    unsafe fn read_unaligned<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            KERNEL_TEXT_START.0;
        assert!(end <= KERNEL_TEXT_END.0,
                "Reading address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START.0;
        core::ptr::read_unaligned(new_addr as *const T)
    }

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T) {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            KERNEL_TEXT_START.0;
        assert!(end <= KERNEL_TEXT_END.0,
                "Writing address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START.0;
        core::ptr::write_volatile(new_addr as *mut T, value)
    }

    // Read a slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + KERNEL_TEXT_START.0;
        assert!(end <= KERNEL_TEXT_END.0,
                "Slicing address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START.0;
        core::slice::from_raw_parts(new_addr as *const T, size)
    }

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + KERNEL_TEXT_START.0;
        assert!(end <= KERNEL_TEXT_END.0,
                "Slicing address '{:?}' is over the kernel text area", paddr);

        let new_addr = paddr.0 + KERNEL_TEXT_START.0;
        core::slice::from_raw_parts_mut(new_addr as *mut T, size)
    }
}

pub struct KernelPhysicalMemory;

impl PhysicalMemory for KernelPhysicalMemory {
    // Translates a physical address to a virtual address
    fn translate(&self, paddr: PhysicalAddress) -> Option<VirtualAddress> {
        // TODO(patrik): Add some checks to that the physical address is
        // inside the bounds of the boot physical memory range
        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;

        Some(VirtualAddress(new_addr))
    }

    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            PHYSICAL_MEMORY_START.0;
        assert!(end < PHYSICAL_MEMORY_END.0,
                "Reading address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;
        core::ptr::read_volatile(new_addr as *const T)
    }

    // Read from physical memory at paddr (can be unalinged)
    unsafe fn read_unaligned<T>(&self, paddr: PhysicalAddress) -> T {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            PHYSICAL_MEMORY_START.0;
        assert!(end < PHYSICAL_MEMORY_END.0,
                "Reading address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;
        core::ptr::read_unaligned(new_addr as *const T)
    }

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T) {
        let end = (paddr.0 + core::mem::size_of::<T>() - 1) +
            PHYSICAL_MEMORY_START.0;
        assert!(end < PHYSICAL_MEMORY_END.0,
                "Writing address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;
        core::ptr::write_volatile(new_addr as *mut T, value)
    }

    // Read a slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + PHYSICAL_MEMORY_START.0;
        assert!(end < PHYSICAL_MEMORY_END.0,
                "Slicing address '{:?}' is over the physical memory area",
                paddr);

        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;
        core::slice::from_raw_parts(new_addr as *const T, size)
    }

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T]
    {
        let byte_length = size * core::mem::size_of::<T>();
        let end = (paddr.0 + byte_length - 1) + PHYSICAL_MEMORY_START.0;
        assert!(end < PHYSICAL_MEMORY_END.0,
                "Slicing address '{:?}' is over the physical memory area",
                paddr);
        let new_addr = paddr.0 + PHYSICAL_MEMORY_START.0;
        core::slice::from_raw_parts_mut(new_addr as *mut T, size)
    }
}
