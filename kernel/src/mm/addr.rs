#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct VirtualAddress(pub usize);

#[derive(Copy, Clone, PartialEq, PartialOrd)]
pub struct PhysicalAddress(pub usize);

impl core::ops::Add<usize> for VirtualAddress {
    type Output = VirtualAddress;

    fn add(self, rhs: usize) -> VirtualAddress {
        VirtualAddress(self.0 + rhs)
    }
}

impl core::ops::Add<usize> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn add(self, rhs: usize) -> PhysicalAddress {
        PhysicalAddress(self.0 + rhs)
    }
}

impl core::fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "PhysicalAddress({:#x})", self.0)
    }
}

impl core::fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "VirtualAddress({:#x})", self.0)
    }
}

