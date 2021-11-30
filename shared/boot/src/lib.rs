//! Library to hold common code for the bootloader like the boot structure
//! passed to the kernel with infomation from the bootloader

#![no_std]

const MAX_MEMORY_MAP_ENTRIES: usize = 64;

pub type BootSize = u64;

#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
pub struct BootPhysicalAddress(u64);

impl BootPhysicalAddress {
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(u64)]
pub enum BootMemoryMapType {
    Available,
    Reserved,
    Acpi,

    Unknown,
}

impl Default for BootMemoryMapType {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(C, align(8))]
pub struct BootMemoryMapEntry {
    /// Starting address of the memory region
    addr:   BootPhysicalAddress,

    /// The length of the memory region
    length: BootSize,

    /// The type of memory region
    typ:    BootMemoryMapType,
}

impl BootMemoryMapEntry {
    pub fn new(addr: BootPhysicalAddress,
               length: BootSize,
               typ: BootMemoryMapType)
        -> Self
    {
        Self {
            addr,
            length,
            typ
        }
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct BootInfo {
    /// Starting address of the heap the kernel can use
    heap_addr: BootPhysicalAddress,

    /// The length of the heap region
    heap_length: BootSize,

    /// Starting address of the initrd
    initrd_addr: BootPhysicalAddress,

    /// The length of the initrd
    initrd_length: BootSize,

    /// Memory Map entries
    memory_map: [BootMemoryMapEntry; MAX_MEMORY_MAP_ENTRIES],

    /// Number of entries used inside the `memory_map`
    num_memory_map_entries: usize,
}

impl BootInfo {
    pub fn new(heap_addr: BootPhysicalAddress, heap_length: BootSize,
               initrd_addr: BootPhysicalAddress, initrd_length: BootSize)
        -> Self
    {
        Self {
            heap_addr,
            heap_length,

            initrd_addr,
            initrd_length,

            memory_map: [BootMemoryMapEntry::default(); MAX_MEMORY_MAP_ENTRIES],
            num_memory_map_entries: 0,
        }
    }

    pub fn add_memory_map_entry(&mut self, entry: BootMemoryMapEntry)
        -> Option<()>
    {
        if self.num_memory_map_entries >= self.memory_map.len() {
            return None;
        }

        self.memory_map[self.num_memory_map_entries] = entry;
        self.num_memory_map_entries += 1;

        Some(())
    }
}
