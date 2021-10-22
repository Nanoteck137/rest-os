use crate::multiboot::{ Multiboot, Tag };
use crate::mm::{ PhysicalAddress, PhysicalMemory, KERNEL_PHYSICAL_MEMORY };

static mut ACPI_TABLE: Option<PhysicalAddress> = None;

#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct SDTHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
}

pub fn initialize(multiboot: &Multiboot) {
    let mut acpi_addr = None;

    for tag in multiboot.tags() {
        match tag {
            Tag::Acpi1(addr) => acpi_addr = Some(addr),
            Tag::Acpi2(addr, _) => acpi_addr = Some(addr),

            _ => {},
        }
    }

    unsafe {
        ACPI_TABLE = acpi_addr;
    }
}

#[derive(Debug)]
pub struct Table {
    header: SDTHeader,
    data_addr: PhysicalAddress,
}

impl Table {
    pub fn header(&self) -> SDTHeader {
        self.header
    }

    pub fn data_addr(&self) -> PhysicalAddress {
        self.data_addr
    }

    pub fn data_length(&self) -> usize {
        self.header.length as usize - core::mem::size_of::<SDTHeader>()
    }
}

pub fn find_table<P>(physical_memory: &P, signature: &[u8; 4])
    -> Option<Table>
    where P: PhysicalMemory
{
    if let Some(acpi_addr) = unsafe { ACPI_TABLE } {
        let acpi_table = unsafe {
            physical_memory.read_unaligned::<SDTHeader>(acpi_addr) };

        let table_length = (acpi_table.length as usize) -
            core::mem::size_of::<SDTHeader>();
        let table_entries = table_length / core::mem::size_of::<u32>();

        let entry_start = acpi_addr.0 + core::mem::size_of::<SDTHeader>();

        for i in 0..table_entries {
            let addr = entry_start + i * core::mem::size_of::<u32>();
            let addr = PhysicalAddress(addr as usize);

            let addr = unsafe { physical_memory.read_unaligned::<u32>(addr) };
            let addr = PhysicalAddress(addr as usize);

            let header = unsafe {
                physical_memory.read_unaligned::<SDTHeader>(addr) };

            let data_addr = addr.0 + core::mem::size_of::<SDTHeader>();
            let data_addr = PhysicalAddress(data_addr);

            if &header.signature == signature {
                return Some(Table {
                    header,
                    data_addr,
                });
            }
        }
    }

    None
}

pub fn debug_dump() {
    let physical_memory = &KERNEL_PHYSICAL_MEMORY;

    if let Some(acpi_addr) = unsafe { ACPI_TABLE } {
        let acpi_table = unsafe {
            physical_memory.read_unaligned::<SDTHeader>(acpi_addr) };

        let table_length = (acpi_table.length as usize) -
            core::mem::size_of::<SDTHeader>();
        let table_entries = table_length / core::mem::size_of::<u32>();

        let entry_start = acpi_addr.0 + core::mem::size_of::<SDTHeader>();

        for i in 0..table_entries {
            let addr = entry_start + i * core::mem::size_of::<u32>();
            let addr = PhysicalAddress(addr as usize);

            let addr = unsafe { physical_memory.read_unaligned::<u32>(addr) };
            let addr = PhysicalAddress(addr as usize);

            let header = unsafe {
                physical_memory.read_unaligned::<SDTHeader>(addr) };

            let data_addr = addr.0 + core::mem::size_of::<SDTHeader>();
            let data_addr = PhysicalAddress(data_addr);

            println!("  - {}",
                     core::str::from_utf8(&header.signature).unwrap());
        }
    }
}
