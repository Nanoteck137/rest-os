use crate::acpi;
use crate::mm;
use crate::mm::MemoryRegionFlags;
use crate::mm::{ PhysicalAddress, PhysicalMemory, KERNEL_PHYSICAL_MEMORY };

use core::sync::atomic::{ AtomicUsize, Ordering };

static NUM_CORES: AtomicUsize = AtomicUsize::new(0);

enum Mode {
}

pub struct Apic {
    mode: Mode,
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

    let addr = mm::map_physical_to_kernel_vm(apic_addr, 4095,
                                             MemoryRegionFlags::READ |
                                             MemoryRegionFlags::WRITE |
                                             MemoryRegionFlags::DISABLE_CACHE);

    let addr = addr.expect("Failed to map in the APIC");
    println!("Addr: {:?}", addr);

    let ptr = addr.0 as *mut u32;
    core::ptr::write_volatile(ptr.offset(0x0f0), (1 << 12) | (1 << 8) | 0xffu32);

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

    Some(())
}
