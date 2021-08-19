use crate::arch;
use crate::arch::x86_64::{ PageTable, PageType };

use crate::multiboot::Multiboot;

use core::convert::TryFrom;

use alloc::sync::{ Arc, Weak };
use alloc::string::String;
use alloc::collections::BTreeMap;

use spin::{ Mutex, RwLock };

pub use frame_alloc::{ FrameAllocator, BitmapFrameAllocator };
pub use heap_alloc::Allocator;

pub use physical_memory::{ PhysicalMemory };
use physical_memory::{ BootPhysicalMemory, KernelPhysicalMemory };

mod heap_alloc;
mod frame_alloc;
mod physical_memory;

pub const PAGE_SIZE: usize = 4096;

pub const KERNEL_TEXT_START: VirtualAddress =
    VirtualAddress(0xffffffff80000000);
pub const KERNEL_TEXT_END: VirtualAddress =
    VirtualAddress(0xffffffffc0000000);
pub const KERNEL_TEXT_SIZE: usize = KERNEL_TEXT_END.0 - KERNEL_TEXT_START.0;

pub const PHYSICAL_MEMORY_START: VirtualAddress =
    VirtualAddress(0xffff888000000000);
pub const PHYSICAL_MEMORY_END: VirtualAddress =
    VirtualAddress(0xffff988000000000);
pub const PHYSICAL_MEMORY_SIZE: usize =
    PHYSICAL_MEMORY_END.0 - PHYSICAL_MEMORY_START.0;

pub const VMALLOC_START: VirtualAddress = VirtualAddress(0xffffa88000000000);
pub const VMALLOC_END: VirtualAddress = VirtualAddress(0xffffb88000000000);
pub const VMALLOC_SIZE: usize = VMALLOC_END.0 - VMALLOC_START.0;

pub static BOOT_PHYSICAL_MEMORY: BootPhysicalMemory = BootPhysicalMemory {};
pub static KERNEL_PHYSICAL_MEMORY: KernelPhysicalMemory =
    KernelPhysicalMemory {};

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

    is_mapped: bool,
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
    multiboot_structure: PhysicalAddress,
    kernel_regions: BTreeMap<usize, Arc<RwLock<VMRegion>>>,

    next_addr: VirtualAddress,

    frame_allocator: BitmapFrameAllocator,

    reference_page_table: PageTable,
}

impl MemoryManager {
    fn create_frame_allocator(multiboot: &Multiboot) -> BitmapFrameAllocator {
        let mut frame_allocator = BitmapFrameAllocator::new();

        let (heap_start, heap_size) = crate::heap();
        let heap_end = heap_start + heap_size;
        let physical_heap_end =
            PhysicalAddress(heap_end.0 - KERNEL_TEXT_START.0);

        unsafe {
            frame_allocator.init(multiboot.find_memory_map()
                .expect("Failed to find memory map"));
        }

        frame_allocator.lock_region(PhysicalAddress(0), 0x4000);

        // TODO(patrik): Change this
        let kernel_start = PhysicalAddress(0x100000);
        let kernel_end = physical_heap_end;
        frame_allocator.lock_region(kernel_start,
                                    kernel_end.0 - kernel_start.0);

        frame_allocator
    }

    fn new(multiboot_structure: PhysicalAddress) -> Self {
        let multiboot = unsafe {
            Multiboot::from_addr(&BOOT_PHYSICAL_MEMORY, multiboot_structure)
        };

        let mut frame_allocator = Self::create_frame_allocator(&multiboot);
        let page_table = PageTable::create(&mut frame_allocator);

        let mut result = Self {
            multiboot_structure,
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

        let multiboot = unsafe {
            Multiboot::from_addr(&BOOT_PHYSICAL_MEMORY,
                                 self.multiboot_structure)
        };

        let page_table = &mut self.reference_page_table;

        unsafe {
            // Search for the highest address inside the memory map so we can
            // map all of the physical memory
            let highest_address = {
                let memory_map = multiboot.find_memory_map()
                    .expect("Failed to find memory map");
                let mut address = 0;
                for entry in memory_map.iter() {
                    let end = entry.addr() + entry.length();
                    address = core::cmp::max(address, end);
                }

                address as usize
            };

            for addr in (KERNEL_TEXT_START.0..KERNEL_TEXT_END.0)
                .step_by(2 * 1024 * 1024)
            {
                let vaddr = VirtualAddress(addr);
                let paddr = PhysicalAddress(addr - KERNEL_TEXT_START.0);
                page_table.map_raw(&mut self.frame_allocator,
                                   &BOOT_PHYSICAL_MEMORY,
                                   vaddr, paddr, PageType::Page2M)
                    .expect("Failed to map");
            }

            // Map all of Physical memory at PHYSICAL_MEMORY_OFFSET
            for offset in (0..=highest_address).step_by(2 * 1024 * 1024) {
                let vaddr = VirtualAddress(offset + PHYSICAL_MEMORY_START.0);
                let paddr = PhysicalAddress(offset);
                page_table.map_raw(&mut self.frame_allocator,
                                   &BOOT_PHYSICAL_MEMORY,
                                   vaddr, paddr, PageType::Page2M)
                    .expect("Failed to map");
            }

            /*
            // Unmap the mappings from 0-1GiB those mappings are from the boot and
            // we need to unmap those
            for offset in (0..1 * 1024 * 1024 * 1024).step_by(2 * 1024 * 1024) {
                page_table.unmap_raw(&mut self.frame_allocator,
                                     &KERNEL_PHYSICAL_MEMORY,
                                     VirtualAddress(offset));
            }
            */
        }

        // TODO(patrik): Free the old page table

        unsafe {
            arch::x86_64::write_cr3(self.reference_page_table.addr().0 as u64);
        }
    }

    /// Creates a page table from the reference page table
    fn create_page_table(&mut self) -> PageTable {
        let page_table = PageTable::create(&mut self.frame_allocator);

        for i in 0..512 {
            unsafe {
                let entry = self.reference_page_table
                    .top_level_entry(&KERNEL_PHYSICAL_MEMORY, i);

                page_table.set_top_level_entry(&KERNEL_PHYSICAL_MEMORY,
                                               i, entry);
            }
        }

        page_table
    }

    fn allocate_kernel_vm(&mut self, name: String, size: usize)
        -> Option<VirtualAddress>
    {
        assert!(size > 0, "Size can't be 0");
        assert!(self.next_addr.0 % PAGE_SIZE == 0);

        let addr = self.next_addr.0;
        let pages = size / PAGE_SIZE + 1;

        self.next_addr.0 += pages * PAGE_SIZE;

        let region = VMRegion {
            name,
            addr: VirtualAddress(addr),
            page_count: pages,

            is_mapped: false,
        };

        let result = region.addr();
        let region = Arc::new(RwLock::new(region));
        self.kernel_regions.insert(addr, region.clone());

        let mut region = region.write();
        self.map_region(&mut region);

        Some(result)
    }

    fn map_in_userspace(&mut self, vaddr: VirtualAddress, size: usize)
        -> Option<()>
    {
        let pages = size / PAGE_SIZE + 1;


        let page_table = &mut self.reference_page_table;

        for page in 0..pages {
            unsafe {
                let frame = self.frame_allocator.alloc_frame()
                    .expect("Failed to allocate frame");

                let vaddr = vaddr + (page * PAGE_SIZE);
                page_table.map_raw_user(&mut self.frame_allocator,
                                   &crate::KERNEL_PHYSICAL_MEMORY,
                                   vaddr + (page * PAGE_SIZE),
                                   PhysicalAddress::from(frame),
                                   PageType::Page4K)
                    .expect("Failed to map");
            }
        }

        Some(())
    }

    fn find_region(&mut self, vaddr: VirtualAddress)
        -> Option<Arc<RwLock<VMRegion>>>
    {
        let vaddr = VirtualAddress(vaddr.0 & !0xfff);
        for region in self.kernel_regions.values() {
            let lock = region.read();

            let start = lock.addr();

            assert!(lock.page_count() != 0);
            let end = lock.addr() + ((lock.page_count() - 1) * PAGE_SIZE);

            if vaddr.0 >= start.0 && vaddr.0 <= end.0 {
                return Some(region.clone());
            }
        }

        None
    }

    fn is_vmalloc_addr(vaddr: VirtualAddress) -> bool {
        vaddr >= VMALLOC_START && vaddr < VMALLOC_END
    }

    fn map_region(&mut self, region: &mut VMRegion) {
        assert!(!region.is_mapped, "Region already mapped");

        let page_table = &mut self.reference_page_table;

        for page in 0..region.page_count() {
            unsafe {
                let frame = self.frame_allocator.alloc_frame()
                    .expect("Failed to allocate frame");

                page_table.map_raw(&mut self.frame_allocator,
                                   &crate::KERNEL_PHYSICAL_MEMORY,
                                   region.addr() + (page * PAGE_SIZE),
                                   PhysicalAddress::from(frame),
                                   PageType::Page4K)
                    .expect("Failed to map");
            }
        }

        region.is_mapped = true;
    }

    fn get_current_page_table() -> PageTable {
        let cr3 = unsafe { arch::x86_64::read_cr3() };

        let page_table =
            unsafe { PageTable::from_table(PhysicalAddress(cr3 as usize)) };

        page_table
    }

    fn page_fault_vmalloc(&mut self, vaddr: VirtualAddress) -> bool {
        let region = self.find_region(vaddr)
            .expect("Failed to find region");
        let mut region = region.write();

        if !region.is_mapped {
            self.map_region(&mut region);
        }

        let page_table = Self::get_current_page_table();

        let (start_p4, _, _, _, _) = PageTable::index(VMALLOC_START);
        let (end_p4, _, _, _, _) = PageTable::index(VMALLOC_END);

        for i in start_p4..end_p4 {
            unsafe {
                let entry = self.reference_page_table.top_level_entry(
                    &KERNEL_PHYSICAL_MEMORY, i);
                page_table.set_top_level_entry(&KERNEL_PHYSICAL_MEMORY,
                                               i, entry);
            }
        }

        true
    }

    fn page_fault(&mut self, vaddr: VirtualAddress) -> bool {
        println!("Page fault: {:?}", vaddr);

        // NOTE(patrik): If the fault is for a vmalloc then we need to map
        // those pages in the current page table maybe even inside the
        // reference page table
        if Self::is_vmalloc_addr(vaddr) {
            return self.page_fault_vmalloc(vaddr);
        }

        false
    }
}

static MM: Mutex<Option<MemoryManager>> = Mutex::new(None);

pub fn initialize(multiboot_structure: PhysicalAddress) {
    *MM.lock() = Some(MemoryManager::new(multiboot_structure));
}

pub fn allocate_kernel_vm(name: String, size: usize) -> Option<VirtualAddress>
{
    MM.lock().as_mut().unwrap().allocate_kernel_vm(name, size)
}

pub fn allocate_kernel_vm_zero(name: String, size: usize)
    -> Option<VirtualAddress>
{
    let res = MM.lock().as_mut().unwrap().allocate_kernel_vm(name, size);

    if let Some(addr) = res {
        unsafe {
            core::ptr::write_bytes(addr.0 as *mut u8, 0, size);
        }
    }

    res
}

pub fn map_in_userspace(vaddr: VirtualAddress, size: usize) -> Option<()> {
    MM.lock().as_mut().unwrap().map_in_userspace(vaddr, size)
}

pub fn page_fault(vaddr: VirtualAddress) -> bool {
    MM.lock().as_mut().unwrap().page_fault(vaddr)
}
