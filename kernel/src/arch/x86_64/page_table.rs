//! Handles all the page table code for creating and modifying page tables

use crate::mm::VirtualAddress;

#[repr(C, packed)]
struct Entry(u64);

#[repr(C, packed)]
struct PageTable {
    entries: [Entry; 512],
}

impl PageTable {
    fn translate(&self) -> Option<VirtualAddress> {
        None
    }

    fn map_direct(&mut self, addr: VirtualAddress, frame: Frame) { }
}
