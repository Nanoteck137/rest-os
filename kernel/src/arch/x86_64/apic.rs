use crate::acpi;
use crate::mm;
use crate::mm::MemoryRegionFlags;
use crate::mm::{ PhysicalAddress, PhysicalMemory, KERNEL_PHYSICAL_MEMORY };
use crate::mm::VirtualAddress;

use core::sync::atomic::{ AtomicUsize, Ordering };
use alloc::boxed::Box;
use spin::RwLock;

const IA32_APIC_BASE_EN: u64 = 1 << 11;
const IA32_APIC_BASE: u32 = 0x1b;

static NUM_CORES: AtomicUsize = AtomicUsize::new(0);

static APIC_ADDR: RwLock<Option<VirtualAddress>> = RwLock::new(None);
static IOAPIC_ADDR: RwLock<Option<VirtualAddress>> = RwLock::new(None);

#[derive(Copy, Clone, Debug)]
pub enum Register {
    ApicId = 0x20,
    EndOfInterrupt = 0xb0,
    SpuriousInterruptVector = 0xf0,

    LvtTimer = 0x320,
    InitialCount = 0x380,
    DivideConfiguration = 0x3e0,
}

pub struct Apic {
    mapping: &'static mut [u32],
}

impl Apic {
    pub unsafe fn eoi(&mut self) {
        self.write_reg(Register::EndOfInterrupt, 0);
    }

    pub unsafe fn read_reg(&self, register: Register) -> u32 {
        let offset = register as usize;

        core::ptr::read_volatile(&self.mapping[offset / 4])
    }

    pub unsafe fn write_reg(&mut self, register: Register, value: u32) {
        let offset = register as usize;

        core::ptr::write_volatile(&mut self.mapping[offset / 4], value)
    }
}

pub(super) fn initialize() {
    if let Some(apic_table) = acpi::find_table(&KERNEL_PHYSICAL_MEMORY, b"APIC") {
        unsafe { parse_madt_table(apic_table) };
    } else {
        println!("Warning: Failed to find APIC/MADT \
                 table from the ACPI Tables");
    }

    println!("Num cores available: {}", NUM_CORES.load(Ordering::SeqCst));
}

unsafe fn parse_madt_table(madt: acpi::Table) -> Option<()> {
    let data_addr = madt.data_addr();
    let start = data_addr;

    let apic_addr = KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start);
    let apic_addr = PhysicalAddress(apic_addr as usize);
    println!("APIC Address: {:#x?}", apic_addr);

    let flags = KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start + 4);

    println!("Flags: {:#x}", flags);

    // Start at the record entries
    let mut start = start + 8;

    loop {
        let typ = KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start);
        let length = KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 1);

        match typ {
            0 => {
                // Local APIC
                let acpi_processor_id =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 2);
                let apic_id =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 3);
                let flags =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start + 4);

                println!("Local APIC: {}, {}, {:#x?}",
                         acpi_processor_id, apic_id, flags);

                if flags & 0x1 == 0x1 || flags & 0x2 == 0x2 {
                    // Core is enabled or Capable of becoming enabled
                    NUM_CORES.fetch_add(1, Ordering::SeqCst);
                } else {
                    panic!("What to do now?");
                }
            },

            1 => {
                // IO APIC

                let io_apic_id =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 2);
                let io_apic_address =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start + 4);
                let global_system_interrupt_base =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start + 8);

                let io_apic_physical_address =
                    PhysicalAddress(io_apic_address as usize);
                let io_apic_addr = mm::map_physical_to_kernel_vm(
                    io_apic_physical_address,
                    4095,
                    MemoryRegionFlags::READ |
                    MemoryRegionFlags::WRITE |
                    MemoryRegionFlags::DISABLE_CACHE);
                let io_apic_addr = io_apic_addr
                    .expect("Failed to map the IOAPIC address to kernel vm");

                {
                    *IOAPIC_ADDR.write() = Some(io_apic_addr);
                }

                println!("IO APIC: {} {:#x} {}",
                         io_apic_id, io_apic_address,
                         global_system_interrupt_base);
            },

            2 => {
                let bus_source =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 2);
                let irq_source =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 3);
                let global_system_interrupt =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u32>(start + 4);
                let flags =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u16>(start + 8);

                println!("IO APIC Interrupt Source Override: {} {} {} {}",
                         bus_source, irq_source,
                         global_system_interrupt, flags);
            },

            3 => {
                unimplemented!();
            },

            4 => {
                let acpi_processor_id =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 2);
                let flags =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u16>(start + 3);
                let lint =
                    KERNEL_PHYSICAL_MEMORY.read_unaligned::<u8>(start + 5);

                println!("Local APIC Non-maskable interrupts: {} {} {}",
                         acpi_processor_id, flags, lint);
            },

            5 => {
                unimplemented!();
            },

            9 => {
                unimplemented!();
            },

            _ => panic!("Unknown MADT entry type: {}", typ),
        }

        start = start + length as usize;

        if start >= data_addr + madt.data_length() {
            break;
        }
    }

    // Enable the APIC
    let apic_base = super::rdmsr(IA32_APIC_BASE);
    super::wrmsr(IA32_APIC_BASE, apic_base | IA32_APIC_BASE_EN);

    let addr = mm::map_physical_to_kernel_vm(apic_addr, 4095,
                                             MemoryRegionFlags::READ |
                                             MemoryRegionFlags::WRITE |
                                             MemoryRegionFlags::DISABLE_CACHE);

    let addr = addr.expect("Failed to map in the APIC");
    println!("Addr: {:?}", addr);

    {
        *APIC_ADDR.write() = Some(addr);
    }

    {
        if let Some(io_apic_addr) = *IOAPIC_ADDR.read() {
            println!("We have a IO APIC: {:?}", io_apic_addr);
        }
    }

    Some(())
}

pub(super) unsafe fn initialize_core(core_id: u32) {
    println!("Initializing APIC for core #{}", core_id);

    if let Some(addr) = *APIC_ADDR.read() {
        let mapping = core::slice::from_raw_parts_mut(addr.0 as *mut u32, 1024);

        let mut apic = Apic {
            mapping
        };

        apic.write_reg(Register::SpuriousInterruptVector, (1 << 8) | 0xff);

        let new = apic.read_reg(Register::SpuriousInterruptVector);
        println!("New value: {:#x?}", new);

        apic.write_reg(Register::DivideConfiguration, 0);
        apic.write_reg(Register::LvtTimer, (1 << 17) | 0xe0);
        apic.write_reg(Register::InitialCount, 500_00000);

        core!().arch().apic = Some(Box::new(apic));
    }
}
