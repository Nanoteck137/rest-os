use crate::multiboot::{ Multiboot, Tag };
use crate::mm::{ PhysicalAddress, PhysicalMemory, KERNEL_PHYSICAL_MEMORY };

static mut ACPI_TABLE: Option<PhysicalAddress> = None;

#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
struct Rsdp {
    signature:         [u8; 8],
    checksum:          u8,
    oem_id:            [u8; 6],
    revision:          u8,
    rsdt_addr:         u32,
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
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

unsafe fn search_acpi<P>(physical_memory: &P)
    -> Option<PhysicalAddress>
    where P: PhysicalMemory
{
    let ebda = physical_memory.read::<u16>(PhysicalAddress(0x40e)) as usize;
    println!("Edba: {:#x?}", ebda);

    let regions = [
        (ebda, ebda + 1024 - 1),

        (0xe0000, 0xfffff)
    ];

    // let mut rsdp = None;

    for &(start, end) in &regions {
        let start = (start + 0xf) & !0xf;

        for paddr in (start..=end).step_by(16) {
            let struct_end = start + core::mem::size_of::<Rsdp>() - 1;

            if struct_end > end {
                break;
            }

            // Read the table
            let table = physical_memory.read::<Rsdp>(PhysicalAddress(paddr));
            if &table.signature != b"RSD PTR " {
                continue;
            }

            println!("Found table: {:#x?}", table);

            return Some(PhysicalAddress(table.rsdt_addr as usize))
        }
    }

    None
}

pub fn initialize<P>(physical_memory: &P, multiboot: &Multiboot)
    where P: PhysicalMemory
{
    let mut acpi_addr = None;

    for tag in multiboot.tags() {
        match tag {
            Tag::Acpi1(addr) => acpi_addr = Some(addr),
            Tag::Acpi2(addr, _) => acpi_addr = Some(addr),

            _ => {},
        }
    }

    acpi_addr = None;

    if acpi_addr.is_none() {
        // NOTE(patrik): The bootloader didn't find the ACPI table so
        // we do a search to find it insteed

        acpi_addr = unsafe { search_acpi(physical_memory) }
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
