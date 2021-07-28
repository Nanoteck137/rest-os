//! Handles all the page table code for creating and modifying page tables
//! Code inspired from: `https://github.com/gamozolabs/chocolate_milk/`

#![allow(dead_code)]

use crate::mm::{ VirtualAddress, PhysicalAddress, PhysicalMemory };
use crate::mm::frame_alloc::FrameAllocator;

use crate::println;

const PAGE_PRESENT:         usize = 1 << 0;
const PAGE_WRITE:           usize = 1 << 1;
const PAGE_USER:            usize = 1 << 2;
const PAGE_CACHE_DISABLE:   usize = 1 << 4;
const PAGE_ACCESSED:        usize = 1 << 5;
const PAGE_DIRTY:           usize = 1 << 6;
const PAGE_HUGE:            usize = 1 << 7;
const PAGE_NX:              usize = 1 << 63;

#[derive(PartialEq)]
pub enum PageType {
    Page4K,
    Page2M,
    Page1G,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct Entry(usize);

impl Entry {
    /// Is the entry present
    fn is_present(&self) -> bool {
        self.0 & PAGE_PRESENT != 0
    }

    /// Set the present flag
    fn set_present(&mut self, value: bool) {
        self.0 &= !PAGE_PRESENT;
        if value {
            self.0 |= PAGE_PRESENT;
        }
    }

    // Is the entry writable
    fn is_writable(&self) -> bool {
        self.0 & PAGE_WRITE != 0
    }

    /// Set the write flag
    fn set_writable(&mut self, value: bool) {
        self.0 &= !PAGE_WRITE;
        if value {
            self.0 |= PAGE_WRITE;
        }
    }

    // Is the entry huge
    fn is_huge(&self) -> bool {
        self.0 & PAGE_HUGE != 0
    }

    /// Set the huge flag
    fn set_huge(&mut self, value: bool) {
        self.0 &= !PAGE_HUGE;
        if value {
            self.0 |= PAGE_HUGE;
        }
    }

    /// Get the address
    fn get_address(&self) -> usize {
        self.0 & 0x000ffffffffff000
    }

    /// Set the address
    fn set_address(&mut self, address: PhysicalAddress) {
        // TODO(patrik): Check the address so it's page aligned
        let address = address.0;

        self.0 |= address;
    }
}

#[derive(Debug)]
pub struct PageMapping {
    p4: Option<PhysicalAddress>,
    p3: Option<PhysicalAddress>,
    p2: Option<PhysicalAddress>,
    p1: Option<PhysicalAddress>,
}

pub struct PageTable {
    table: PhysicalAddress
}

impl PageTable {
    pub unsafe fn from_table(table: PhysicalAddress) -> Self {
        Self {
            table: PhysicalAddress(table.0 & !0xfff)
        }
    }

    /// Translates a ´vaddr´ and returns the mapping tables for
    /// that address
    pub unsafe fn translate_mapping<P>(&self,
                                       physical_memory: &P,
                                       vaddr: VirtualAddress)
        -> Option<PageMapping>

        where P: PhysicalMemory
    {
        let mut result = PageMapping {
            p4: None,
            p3: None,
            p2: None,
            p1: None,
        };

        let (p4, p3, p2, p1, _) = PageTable::index(vaddr);

        let indicies = [
            p4, p3, p2, p1
        ];

        let mut table = self.table;

        for (depth, &index) in indicies.iter().enumerate() {
            println!("Table: {:#x?}", table);
            let entry_off = index * core::mem::size_of::<Entry>();
            let entry_addr = PhysicalAddress(table.0 + entry_off);

            match depth {
                0 => result.p4 = Some(entry_addr),
                1 => result.p3 = Some(entry_addr),
                2 => result.p2 = Some(entry_addr),
                3 => result.p1 = Some(entry_addr),

                _ => unreachable!(),
            }

            let entry = physical_memory.read::<Entry>(entry_addr);

            if !entry.is_present() {
                break;
            }

            table = PhysicalAddress(entry.get_address());

            if depth == 3 || entry.is_huge() {
                break;
            }
        }

        Some(result)
    }

    pub unsafe fn map<F, P>(&mut self,
                            frame_allocator: &mut F, physical_memory: &P,
                            vaddr: VirtualAddress,
                            paddr: PhysicalAddress,
                            page_type: PageType)
        -> Option<()>

        where F: FrameAllocator,
              P: PhysicalMemory
    {
        let mapping = self.translate_mapping(physical_memory, vaddr)?;

        let mut entries = [
            mapping.p4,
            mapping.p3,
            mapping.p2,
            mapping.p1,
        ];

        let depth = match page_type {
            PageType::Page1G => 2,
            PageType::Page2M => 3,
            PageType::Page4K => 4,
        };

        let (p4, p3, p2, p1, _) = PageTable::index(vaddr);
        let indicies = [
            p4, p3, p2, p1
        ];

        if entries.get(depth).map_or(false, |x| x.is_some()) {
            return None;
        }

        for index in 1..depth {
            if entries[index].is_none() {
                let new_table = frame_allocator.alloc_frame()?;
                let new_table = PhysicalAddress::from(new_table);
                println!("Allocating new table: {:?}", new_table);

                let addr = entries[index - 1].unwrap();

                let mut new_entry = Entry(0);
                new_entry.set_address(new_table);
                new_entry.set_present(true);
                new_entry.set_writable(true);
                physical_memory.write::<Entry>(addr, new_entry);

                entries[index] = Some(PhysicalAddress(
                    new_table.0 +
                        indicies[index] * core::mem::size_of::<Entry>()
                ));
            }
        }

        let mut entry = Entry(0);
        entry.set_address(paddr);
        entry.set_present(true);
        entry.set_writable(true);
        if page_type != PageType::Page4K {
            entry.set_huge(true);
        }
        println!("New Entry: {:#x?}", entry);
        println!("Target: {:?}", entries[depth - 1].unwrap());
        physical_memory.write::<Entry>(entries[depth - 1].unwrap(), entry);

        Some(())
    }

    fn index(addr: VirtualAddress) -> (usize, usize, usize, usize, usize) {
        let offset = addr.0 & 0xfff;
        let p1 = (addr.0 >> 12) & 0x1ff;
        let p2 = (addr.0 >> 21) & 0x1ff;
        let p3 = (addr.0 >> 30) & 0x1ff;
        let p4 = (addr.0 >> 39) & 0x1ff;

        (p4, p3, p2, p1, offset)
    }
}
