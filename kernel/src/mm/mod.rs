use core::convert::TryFrom;

pub mod heap_alloc;
pub mod frame_alloc;

pub const PAGE_SIZE: usize = 4096;

#[derive(Copy, Clone, PartialEq)]
pub struct VirtualAddress(pub usize);

impl core::fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtualAddress({:#x})", self.0)
    }
}

impl core::ops::Add<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn add(self, rhs: usize) -> VirtualAddress {
        VirtualAddress(self.0 + rhs)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub struct PhysicalAddress(pub usize);

impl From<Frame> for PhysicalAddress {
    fn from(value: Frame) -> Self {
        Self(value.index * PAGE_SIZE)
    }
}

impl core::fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalAddress({:#x})", self.0)
    }
}

pub trait PhysicalMemory
{
    // Read from physical memory
    unsafe fn read<T>(&self, paddr: PhysicalAddress) -> T;

    // Write to physical memory
    unsafe fn write<T>(&self, paddr: PhysicalAddress, value: T);

    // Slice from physical memory
    unsafe fn slice<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a [T];

    // Mutable Slice from physical memory
    unsafe fn slice_mut<'a, T>(&self, paddr: PhysicalAddress, size: usize)
        -> &'a mut [T];
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Frame {
    index: usize
}

impl TryFrom<PhysicalAddress> for Frame {
    type Error = ();

    fn try_from(addr: PhysicalAddress) -> Result<Self, Self::Error> {
        if addr.0 % 4096 != 0 {
            return Err(());
        }

        Ok(Self {
            index: addr.0 / 4096
        })
    }
}
