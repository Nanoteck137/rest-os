use crate::arch;
use crate::arch::x86_64::{ PageTable, PageType };

use crate::multiboot::Multiboot;
// use crate::process::{ Task, MemorySpace, MemoryRegionFlags };

use core::convert::TryFrom;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::sync::{ Arc, Weak };
use alloc::collections::BTreeMap;

use spin::{ Mutex, RwLock, RwLockWriteGuard };

pub use frame_alloc::{ FrameAllocator, BitmapFrameAllocator };
pub use heap_alloc::Allocator;

pub use physical_memory::{ PhysicalMemory };
use physical_memory::{ BootPhysicalMemory, KernelPhysicalMemory };

pub use addr::{ VirtualAddress, PhysicalAddress };

mod heap_alloc;
mod frame_alloc;
mod physical_memory;
mod addr;

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

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Frame {
    index: usize
}

impl Frame {
    pub fn from_paddr(paddr: PhysicalAddress) -> Self {
        let frame = (paddr.0 & !0xfff) / PAGE_SIZE;

        Self {
            index: frame
        }
    }

    pub fn paddr(&self) -> PhysicalAddress {
        PhysicalAddress(self.index * PAGE_SIZE)
    }
}

impl core::ops::Add<usize> for Frame {
    type Output = Frame;

    fn add(self, rhs: usize) -> Frame {
        Frame {
            index: self.index + rhs
        }
    }
}

bitflags! {
    pub struct MemoryRegionFlags: u32 {
        const READ          = 1 << 0;
        const WRITE         = 1 << 1;
        const EXECUTE       = 1 << 2;
        const DISABLE_CACHE = 1 << 3;
    }
}

#[derive(Debug)]
struct MemoryRegion {
    addr: VirtualAddress,
    size: usize,
    flags: MemoryRegionFlags,
}

impl MemoryRegion {
    fn new(addr: VirtualAddress, size: usize, flags: MemoryRegionFlags)
        -> Self
    {
        Self {
            addr,
            size,
            flags
        }
    }
}

#[derive(Debug)]
pub struct MemorySpace {
    regions: Vec<MemoryRegion>,
    page_table: PageTable,
}

impl MemorySpace {
    pub fn new() -> Self {
        let page_table = create_page_table();

        Self {
            regions: Vec::new(),
            page_table,
        }
    }

    fn add_region(&mut self,
                  vaddr: VirtualAddress, size: usize,
                  flags: MemoryRegionFlags)
    {
        // TODO(patrik): Check for overlap

        self.regions.push(MemoryRegion::new(vaddr, size, flags));
    }

    pub fn page_table(&self) -> &PageTable {
        &self.page_table
    }

    pub fn page_table_mut(&mut self) -> &mut PageTable {
        &mut self.page_table
    }
}

#[derive(Debug)]
pub struct VMRegion {
    name: Option<String>,

    // pages: Vec<Page>,
    vaddr: VirtualAddress,
    paddr: Option<PhysicalAddress>,
    page_count: usize,

    flags: MemoryRegionFlags,

    is_mapped: bool,
}

impl VMRegion {
    pub fn create_with_paddr(vaddr: VirtualAddress,
                             paddr: PhysicalAddress,
                             page_count: usize,
                             flags: MemoryRegionFlags)
        -> Self
    {
        Self {
            name: None,

            vaddr,
            paddr: Some(paddr),
            page_count,

            flags,

            is_mapped: false,
        }
    }

    pub fn create_with_name(name: String,
                            vaddr: VirtualAddress,
                            page_count: usize,
                            flags: MemoryRegionFlags)
        -> Self
    {
        Self {
            name: Some(name),

            vaddr,
            paddr: None,
            page_count,

            flags,

            is_mapped: false,
        }
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn vaddr(&self) -> VirtualAddress {
        self.vaddr
    }

    pub fn paddr(&self) -> Option<PhysicalAddress> {
        self.paddr
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    pub fn flags(&self) -> MemoryRegionFlags {
        self.flags
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
        verify_interrupts_disabled!();

        // TODO(patrik): We need to refactor this code
        //  - Create a list of ranges for the memory map

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
        verify_interrupts_disabled!();

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
                                   vaddr, paddr, PageType::Page2M,
                                   MemoryRegionFlags::READ |
                                   MemoryRegionFlags::WRITE |
                                   MemoryRegionFlags::EXECUTE)
                    .expect("Failed to map");
            }

            // Map all of Physical memory at PHYSICAL_MEMORY_OFFSET
            for offset in (0..=highest_address).step_by(2 * 1024 * 1024) {
                let vaddr = VirtualAddress(offset + PHYSICAL_MEMORY_START.0);
                let paddr = PhysicalAddress(offset);
                page_table.map_raw(&mut self.frame_allocator,
                                   &BOOT_PHYSICAL_MEMORY,
                                   vaddr, paddr, PageType::Page2M,
                                   MemoryRegionFlags::READ |
                                   MemoryRegionFlags::WRITE)
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

        // TODO(patrik): Let the page table handle the copying
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

        let vaddr = self.next_addr.0;
        let page_count = size / PAGE_SIZE + 1;

        self.next_addr.0 += page_count * PAGE_SIZE;

        let region = VMRegion::create_with_name(name,
                                                VirtualAddress(vaddr),
                                                page_count,
                                                MemoryRegionFlags::READ |
                                                MemoryRegionFlags::WRITE);

        let result = region.vaddr();
        let region = Arc::new(RwLock::new(region));
        self.kernel_regions.insert(vaddr, region.clone());

        let mut region = region.write();
        self.map_region(&mut region);

        Some(result)
    }

    fn map_in_userspace(&mut self,
                        memory_space: &mut MemorySpace,
                        vaddr: VirtualAddress, size: usize,
                        flags: MemoryRegionFlags)
        -> Option<()>
    {
        let pages = size / PAGE_SIZE + 1;

        // let page_table = &mut self.reference_page_table;

        let page_table = memory_space.page_table_mut();

        for page in 0..pages {
            unsafe {
                let frame = self.frame_allocator.alloc_frame()
                    .expect("Failed to allocate frame");

                let vaddr = vaddr + (page * PAGE_SIZE);
                page_table.map_raw_user(&mut self.frame_allocator,
                                   &crate::KERNEL_PHYSICAL_MEMORY,
                                   vaddr,
                                   frame.paddr(),
                                   PageType::Page4K,
                                   flags)
                    .expect("Failed to map");
            }
        }

        memory_space.add_region(vaddr, size, flags);

        Some(())
    }

    fn map_physical_to_kernel_vm(&mut self,
                                 paddr: PhysicalAddress, size: usize,
                                 flags: MemoryRegionFlags)
        -> Option<VirtualAddress>
    {
        assert!(size > 0, "Size can't be 0");
        assert!(self.next_addr.0 % PAGE_SIZE == 0);

        let vaddr = self.next_addr.0;
        let page_count = size / PAGE_SIZE + 1;

        self.next_addr.0 += page_count * PAGE_SIZE;

        let region = VMRegion::create_with_paddr(VirtualAddress(vaddr),
                                                 paddr,
                                                 page_count,
                                                 flags);

        let result = region.vaddr();
        let region = Arc::new(RwLock::new(region));
        self.kernel_regions.insert(vaddr, region.clone());

        let mut region = region.write();
        self.map_region(&mut region);

        Some(result)
    }

    fn find_region(&mut self, vaddr: VirtualAddress)
        -> Option<Arc<RwLock<VMRegion>>>
    {
        let vaddr = VirtualAddress(vaddr.0 & !0xfff);
        for region in self.kernel_regions.values() {
            let lock = region.read();

            let start = lock.vaddr();

            assert!(lock.page_count() != 0);
            let end = lock.vaddr() + ((lock.page_count() - 1) * PAGE_SIZE);

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
        let page_table = &mut self.reference_page_table;

        for offset in 0..region.page_count() {
            unsafe {
                let frame = if let Some(paddr) = region.paddr() {
                    Frame::from_paddr(paddr) + offset
                } else {
                    self.frame_allocator.alloc_frame()
                        .expect("Failed to allocate frame")
                };

                page_table.map_raw(&mut self.frame_allocator,
                                   &KERNEL_PHYSICAL_MEMORY,
                                   region.vaddr() + (offset * PAGE_SIZE),
                                   frame.paddr(),
                                   PageType::Page4K,
                                   region.flags())
                    .expect("Failed to map");
            }
        }
    }

    fn get_current_page_table<'a>()  {
        // core!().task().write().memory_space().write()
    }

    fn page_fault_vmalloc(&mut self, vaddr: VirtualAddress) -> bool {
        let process = core!().process();
        let mut process_lock = process.write();

        let memory_space = process_lock.memory_space_mut()
            .expect("Process has no memory space");
        let page_table = memory_space.page_table_mut();

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
        // println!("Page fault: {:?}", vaddr);

        // NOTE(patrik): If the fault is for a vmalloc then we need to map
        // those pages in the current page table maybe even inside the
        // reference page table
        if Self::is_vmalloc_addr(vaddr) {
            return self.page_fault_vmalloc(vaddr);
        }

        false
    }

    fn kernel_task_cr3(&self) -> u64 {
        self.reference_page_table.addr().0 as u64
    }
}

static MM: Mutex<Option<MemoryManager>> = Mutex::new(None);

pub fn initialize(multiboot_structure: PhysicalAddress) {
    {
        let mut lock = MM.lock();
        assert!(lock.is_none(), "MM: Memory Manager already initialized");

        *lock = Some(MemoryManager::new(multiboot_structure));
    }
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

pub fn map_physical_to_kernel_vm(paddr: PhysicalAddress, size: usize,
                                 flags: MemoryRegionFlags)
    -> Option<VirtualAddress>
{
    MM.lock().as_mut().unwrap().map_physical_to_kernel_vm(paddr, size, flags)
}

pub fn map_in_userspace(memory_space: &mut MemorySpace,
                        vaddr: VirtualAddress, size: usize,
                        flags: MemoryRegionFlags)
    -> Option<()>
{
    MM.lock().as_mut().unwrap().map_in_userspace(memory_space,
                                                 vaddr, size, flags)
}

pub fn page_fault(vaddr: VirtualAddress) -> bool {
    MM.lock().as_mut().unwrap().page_fault(vaddr)
}

pub fn create_page_table() -> PageTable {
    MM.lock().as_mut().unwrap().create_page_table()
}

// TODO(patrik): Remove this and find a better way to initialize a kernel task
// cr3 register
pub fn kernel_task_cr3() -> u64 {
    MM.lock().as_mut().unwrap().kernel_task_cr3()
}
