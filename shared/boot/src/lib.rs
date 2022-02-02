//! Library to hold common code for the bootloader like the boot structure
//! passed to the kernel with infomation from the bootloader

#![no_std]

const MAX_MEMORY_MAP_ENTRIES: usize = 128;

pub type BootSize = u64;

fn overlaps(mut x1: u64, mut x2: u64, mut y1: u64, mut y2: u64) -> bool {
    if x1 > x2 {
        core::mem::swap(&mut x1, &mut x2);
    }

    if y1 > y2 {
        core::mem::swap(&mut y1, &mut y2);
    }

    if x1 <= y2 && y1 <= x2 {
        return true;
    }

    false
}

#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
pub struct BootPhysicalAddress(u64);

impl BootPhysicalAddress {
    pub fn new(addr: u64) -> Self {
        Self(addr)
    }

    pub fn raw(&self) -> u64 {
        self.0
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
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

    pub fn addr(&self) -> BootPhysicalAddress {
        self.addr
    }

    pub fn length(&self) -> BootSize {
        self.length
    }

    pub fn typ(&self) -> BootMemoryMapType {
        self.typ
    }

    pub fn overlaps(&self, other: Self) -> bool {
        return overlaps(self.addr.raw(), self.addr.raw() + self.length,
                        other.addr.raw(), other.addr.raw() + other.length);
    }
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct BootInfo {
    /// The start of the kernel
    kernel_start: BootPhysicalAddress,

    /// The next byte over the kernel end
    kernel_end: BootPhysicalAddress,

    /// Starting address of the initrd
    initrd_addr: BootPhysicalAddress,

    /// The length of the initrd
    initrd_length: BootSize,

    /// Memory Map entries
    memory_map: [BootMemoryMapEntry; MAX_MEMORY_MAP_ENTRIES],

    /// Number of entries used inside the `memory_map`
    pub num_memory_map_entries: usize,

    /// The address of the ACPI RSDP
    acpi_table: BootPhysicalAddress,
}

impl BootInfo {
    pub fn new(kernel_start: BootPhysicalAddress,
               kernel_end: BootPhysicalAddress,
               initrd_addr: BootPhysicalAddress,
               initrd_length: BootSize,
               acpi_table: BootPhysicalAddress)
        -> Self
    {
        Self {
            kernel_start,
            kernel_end,

            initrd_addr,
            initrd_length,

            memory_map: [BootMemoryMapEntry::default(); MAX_MEMORY_MAP_ENTRIES],
            num_memory_map_entries: 0,

            acpi_table
        }
    }

    pub fn kernel_start(&self) -> BootPhysicalAddress {
        self.kernel_start
    }

    pub fn kernel_end(&self) -> BootPhysicalAddress {
        self.kernel_end
    }

    pub fn initrd_addr(&self) -> BootPhysicalAddress {
        self.initrd_addr
    }

    pub fn initrd_length(&self) -> BootSize {
        self.initrd_length
    }

    pub fn memory_map(&self) -> &[BootMemoryMapEntry] {
        &self.memory_map[..self.num_memory_map_entries]
    }

    pub fn acpi_table(&self) -> BootPhysicalAddress {
        self.acpi_table
    }

    pub fn add_memory_map_entry(&mut self, mut entry: BootMemoryMapEntry)
        -> Option<()>
    {
        if self.num_memory_map_entries >= self.memory_map.len() {
            return None;
        }

        'merge: loop {
            for index in 0..self.num_memory_map_entries {
                let ent = self.memory_map[index];

                if entry.typ() != ent.typ() {
                    continue;
                }

                if !ent.overlaps(entry) {
                    continue;
                }

                let addr = core::cmp::min(entry.addr.raw(), ent.addr.raw());
                entry.addr = BootPhysicalAddress(addr);
                entry.length += ent.length;

                self.delete_memory_map_entry(index);

                continue 'merge;
            }

            break;
        }

        self.memory_map[self.num_memory_map_entries] = entry;
        self.num_memory_map_entries += 1;

        Some(())
    }

    fn delete_memory_map_entry(&mut self, idx: usize) {
        for index in idx..self.num_memory_map_entries - 1 {
            self.memory_map.swap(index, index + 1);
        }

        self.num_memory_map_entries -= 1;
    }
}
