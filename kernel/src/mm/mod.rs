use crate::arch;
use crate::arch::x86_64::{ PageTable, PageType };

use core::convert::TryFrom;

use alloc::sync::{ Arc, Weak };
use alloc::string::String;
use alloc::collections::BTreeMap;

use spin::Mutex;

use frame_alloc::{ FrameAllocator, BitmapFrameAllocator };

pub mod heap_alloc;
pub mod frame_alloc;

pub const PAGE_SIZE: usize = 4096;

pub const PHYSICAL_MEMORY_START: VirtualAddress =
    VirtualAddress(0xffff888000000000);
pub const PHYSICAL_MEMORY_END: VirtualAddress =
    VirtualAddress(0xffff988000000000);
pub const PHYSICAL_MEMORY_SIZE: usize =
    PHYSICAL_MEMORY_END.0 - PHYSICAL_MEMORY_START.0;

pub const VMALLOC_START: VirtualAddress = VirtualAddress(0xffffa88000000000);
pub const VMALLOC_END: VirtualAddress = VirtualAddress(0xffffb88000000000);
pub const VMALLOC_SIZE: usize = VMALLOC_END.0 - VMALLOC_START.0;

#[derive(Copy, Clone, PartialEq, PartialOrd)]
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

#[derive(Copy, Clone, PartialEq, PartialOrd)]
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
    // Translates a physical address to a virtual address
    fn translate(&self, paddr: PhysicalAddress) -> Option<VirtualAddress>;

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

#[derive(Debug)]
pub struct VMRegion {
    name: String,
    addr: VirtualAddress,
    page_count: usize,
}

impl VMRegion {
    pub fn addr(&self) -> VirtualAddress {
        self.addr
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }
}

struct MemoryManager {
    kernel_regions: BTreeMap<usize, Arc<VMRegion>>,

    next_addr: VirtualAddress,

    frame_allocator: BitmapFrameAllocator,

    reference_page_table: PageTable,
}

impl MemoryManager {
    fn new(mut frame_allocator: BitmapFrameAllocator) -> Self {
        let page_table = PageTable::create(&mut frame_allocator);

        let mut result = Self {
            kernel_regions: BTreeMap::new(),
            next_addr: VMALLOC_START,
            frame_allocator,

            reference_page_table: page_table,
        };

        result.initialize();

        result
    }

    fn initialize(&mut self) {
        // TODO(patrik): Initialize the reference page table
        //   - The reference page table is used to create new page table
        //     with the kernel mappings identical
        // TODO(patrik): Map in the kernel text
        // TODO(patrik): Map in physical memory
    }

    fn allocate_kernel_vm(&mut self, name: String, size: usize)
        -> Option<Weak<VMRegion>>
    {
        assert!(self.next_addr.0 % PAGE_SIZE == 0);

        let addr = self.next_addr.0;
        let pages = size / PAGE_SIZE + 1;

        self.next_addr.0 += pages * PAGE_SIZE;

        let region = Arc::new(VMRegion {
            name,
            addr: VirtualAddress(addr),
            page_count: pages
        });

        let result = Arc::downgrade(&region);
        self.kernel_regions.insert(addr, region);

        Some(result)
    }

    fn find_region(&mut self, vaddr: VirtualAddress) -> Option<Arc<VMRegion>> {
        for region in self.kernel_regions.values() {
            let start = region.addr();
            let end = region.addr() + (region.page_count() * PAGE_SIZE);

            if start >= vaddr && vaddr < end {
                return Some(region.clone());
            }

            println!("Start: {:?} End: {:?}", start, end);
        }

        None
    }

    fn is_vmalloc_addr(vaddr: VirtualAddress) -> bool {
        if vaddr >= VMALLOC_START && vaddr < VMALLOC_END {
            true
        } else {
            false
        }
    }

    fn page_fault_vmalloc(&mut self, vaddr: VirtualAddress) -> bool {
        let region = self.find_region(vaddr)
            .expect("Failed to find region");

        println!("Region: {:?}", region);
        let cr3 = unsafe { arch::x86_64::read_cr3() };
        println!("CR3: {:#x}", cr3);

        let mut page_table =
            unsafe { PageTable::from_table(PhysicalAddress(cr3 as usize)) };

        for page in 0..region.page_count() {
            unsafe {
                let frame = self.frame_allocator.alloc_frame()
                    .expect("Failed to allocate frame");
                println!("Target: {:?}", PhysicalAddress::from(frame));

                page_table.map_raw(&mut self.frame_allocator,
                                   &crate::KERNEL_PHYSICAL_MEMORY,
                                   region.addr() + (page * PAGE_SIZE),
                                   PhysicalAddress::from(frame),
                                   PageType::Page4K)
                    .expect("Failed to map");
            }
        }

        true
    }

    fn page_fault(&mut self, vaddr: VirtualAddress) -> bool {
        println!("Page fault: {:?}", vaddr);

        if Self::is_vmalloc_addr(vaddr) {
            return self.page_fault_vmalloc(vaddr);
        }

        true
    }
}

static MM: Mutex<Option<MemoryManager>> = Mutex::new(None);

pub fn initialize(frame_allocator: BitmapFrameAllocator) {
    *MM.lock() = Some(MemoryManager::new(frame_allocator));
}

pub fn allocate_kernel_vm(name: String, size: usize) -> Option<Weak<VMRegion>> {
    assert!(size > 0, "Size can't be 0");

    // Allocate from the kernel vmalloc region
    // Map in the region

    MM.lock().as_mut().unwrap().allocate_kernel_vm(name, size)
}

pub fn page_fault(vaddr: VirtualAddress) -> bool {
    MM.lock().as_mut().unwrap().page_fault(vaddr)
}
