//! Handles all the page table code for creating and modifying page tables
//! Code inspired from: `https://github.com/gamozolabs/chocolate_milk/`
//! TODO(patrik):
//!   * Make errors better
//!   * When mapping check the vaddr and paddr so thay are aligned

#![allow(dead_code)]

use crate::mm::{ VirtualAddress, PhysicalAddress, PhysicalMemory, Frame };
use crate::mm::{ FrameAllocator, PAGE_SIZE };

use core::convert::TryFrom;

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

    // Is the entry user accessible
    fn is_user(&self) -> bool {
        self.0 & PAGE_USER != 0
    }

    /// Set the user accessible flag
    fn set_user(&mut self, value: bool) {
        self.0 &= !PAGE_USER;
        if value {
            self.0 |= PAGE_USER;
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

#[derive(Debug)]
pub struct PageTable {
    table: PhysicalAddress
}

impl PageTable {
    pub fn create<F>(frame_allocator: &mut F) -> Self
        where F: FrameAllocator
    {
        let frame = frame_allocator.alloc_frame()
            .expect("Failed to allocate frame for the new page table");

        let paddr = PhysicalAddress::from(frame);

        Self {
            table: paddr
        }
    }

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

    pub unsafe fn map_raw<F, P>(&mut self,
                                frame_allocator: &mut F, physical_memory: &P,
                                vaddr: VirtualAddress,
                                paddr: PhysicalAddress,
                                page_type: PageType)
        -> Option<()>

        where F: FrameAllocator,
              P: PhysicalMemory
    {
        self.map_raw_option(frame_allocator, physical_memory,
                            vaddr, paddr, page_type, false)
    }

    pub unsafe fn map_raw_user<F, P>(&mut self,
                                     frame_allocator: &mut F, physical_memory: &P,
                                     vaddr: VirtualAddress,
                                     paddr: PhysicalAddress,
                                     page_type: PageType)
        -> Option<()>

        where F: FrameAllocator,
              P: PhysicalMemory
    {
        self.map_raw_option(frame_allocator, physical_memory,
                            vaddr, paddr, page_type, true)
    }

    pub unsafe fn map_raw_option<F, P>(&mut self,
                                       frame_allocator: &mut F, physical_memory: &P,
                                       vaddr: VirtualAddress,
                                       paddr: PhysicalAddress,
                                       page_type: PageType,
                                       user: bool)
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
                let ptr = physical_memory.translate(new_table)
                    .expect("Failed to translate addr");
                core::ptr::write_bytes(ptr.0 as *mut u8, 0, 4096);

                let addr = entries[index - 1].unwrap();

                let mut new_entry = Entry(0);
                new_entry.set_address(new_table);
                new_entry.set_present(true);
                new_entry.set_writable(true);
                new_entry.set_user(user);
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
        entry.set_user(user);
        if page_type != PageType::Page4K {
            entry.set_huge(true);
        }

        physical_memory.write::<Entry>(entries[depth - 1].unwrap(), entry);

        Some(())
    }

    unsafe fn can_free_table<P>(physical_memory: &P,
                                table_addr: PhysicalAddress)
        -> bool
        where P: PhysicalMemory
    {
        assert!(table_addr.0 % PAGE_SIZE == 0,
                "table_addr: need to be page aligned");
        let mut result = true;

        for i in 0..512 {
            let entry_off = i * core::mem::size_of::<Entry>();
            let entry_addr = PhysicalAddress(table_addr.0 + entry_off);
            let entry = physical_memory.read::<Entry>(entry_addr);

            if entry.is_present() {
                result = false;
                break;
            }
        }

        result
    }

    pub unsafe fn check_free_table<F, P>(frame_allocator: &mut F,
                                         physical_memory: &P,
                                         table_addr: PhysicalAddress)
        -> bool

        where F: FrameAllocator,
              P: PhysicalMemory
    {
        let table_addr = PhysicalAddress(table_addr.0 & !0xfff);
        if Self::can_free_table(physical_memory, table_addr) {
            let frame = Frame::try_from(table_addr)
                .expect("Failed to convert to Frame");
            frame_allocator.free_frame(frame);

            return true;
        }

        false
    }

    unsafe fn invalidate_page(vaddr: VirtualAddress) {
        asm!("invlpg [{}]", in(reg) vaddr.0);
    }

    pub unsafe fn unmap_raw<F, P>(&mut self,
                                  frame_allocator: &mut F, physical_memory: &P,
                                  vaddr: VirtualAddress)
        -> Option<()>

        where F: FrameAllocator,
              P: PhysicalMemory
    {
        let mapping = self.translate_mapping(physical_memory, vaddr)?;
        assert!(mapping.p2.is_some(), "No support for 1GiB mapping");

        if let Some(p1) = mapping.p1 {
            let mut entry = physical_memory.read::<Entry>(p1);
            entry.set_present(false);
            physical_memory.write::<Entry>(p1, entry);

            Self::invalidate_page(vaddr);

            let p2 = mapping.p2.expect("No P2 table?");
            let p3 = mapping.p3.expect("No P3 table?");
            let p4 = mapping.p4.expect("No P4 table?");

            let mappings = [
                p1, p2, p3, p4
            ];

            for i in 0..mappings.len() - 1 {
                let current_mapping = mappings[i];
                let next_mapping = mappings[i + 1];
                if Self::check_free_table(frame_allocator, physical_memory,
                                          current_mapping)
                {
                    let mut entry =
                        physical_memory.read::<Entry>(next_mapping);
                    entry.set_present(false);
                    physical_memory.write::<Entry>(next_mapping, entry);
                }
            }
        } else if let Some(p2) = mapping.p2 {
            let mut entry = physical_memory.read::<Entry>(p2);
            entry.set_present(false);
            physical_memory.write::<Entry>(p2, entry);

            Self::invalidate_page(vaddr);

            let p3 = mapping.p3.expect("No P3 table?");
            let p4 = mapping.p4.expect("No P4 table?");

            let mappings = [
                p2, p3, p4
            ];

            for i in 0..mappings.len() - 1 {
                let current_mapping = mappings[i];
                let next_mapping = mappings[i + 1];
                if Self::check_free_table(frame_allocator, physical_memory,
                                          current_mapping)
                {
                    let mut entry =
                        physical_memory.read::<Entry>(next_mapping);
                    entry.set_present(false);
                    physical_memory.write::<Entry>(next_mapping, entry);
                }
            }
        }

        Some(())
    }

    pub unsafe fn set_top_level_entry<P>(&self, physical_memory: &P,
                                         index: usize, entry: Entry)
        where P: PhysicalMemory
    {
        let addr = self.table.0 + index * core::mem::size_of::<Entry>();
        let addr = PhysicalAddress(addr);
        physical_memory.write::<Entry>(addr, entry);
    }

    pub unsafe fn top_level_entry<P>(&self, physical_memory: &P, index: usize)
        -> Entry
        where P: PhysicalMemory
    {
        let addr = self.table.0 + index * core::mem::size_of::<Entry>();
        let addr = PhysicalAddress(addr);
        let entry = physical_memory.read::<Entry>(addr);

        entry
    }

    pub fn index(addr: VirtualAddress) -> (usize, usize, usize, usize, usize) {
        let offset = addr.0 & 0xfff;
        let p1 = (addr.0 >> 12) & 0x1ff;
        let p2 = (addr.0 >> 21) & 0x1ff;
        let p3 = (addr.0 >> 30) & 0x1ff;
        let p4 = (addr.0 >> 39) & 0x1ff;

        (p4, p3, p2, p1, offset)
    }

    pub fn addr(&self) -> PhysicalAddress {
        self.table
    }
}
