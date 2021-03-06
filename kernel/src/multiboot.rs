//! This module handles the Multiboot Structure and parses all the diffrent
//! tags and gives the user a interface to access the tag data
//! for example the memory map
//!
//! TODO(patrik):
//!   * Fix all the memory access this module does
//!   * Implement all the tag from the Multiboot2 spec
//!   * Cleanup the code
//!     * Get accessors

#![allow(dead_code)]

use core::convert::TryInto;
use alloc::string::String;
use alloc::string::ToString;

use crate::mm::{ PhysicalMemory, PhysicalAddress };

#[derive(Debug)]
pub enum Tag<'a> {
    CommandLine(&'a str),
    BootloaderName(&'a str),
    Module(Module<'a>),
    BasicMemInfo(u32, u32),
    BootDev(BootDev),
    MemoryMap(MemoryMap<'a>),
    Framebuffer(Framebuffer),
    ElfSections(ElfSections<'a>),
    Acpi1(PhysicalAddress),
    Acpi2(PhysicalAddress, usize),
    LoadBaseAddr(usize),
    Unknown(u32),
}

#[derive(Debug)]
pub struct Module<'a> {
    start: u32,
    end: u32,
    name: &'a str
}

impl<'a> Module<'a> {
    fn new(start: u32, end: u32, name: &'a str) -> Self {
        Self {
            start,
            end,
            name,
        }
    }

    pub fn start(&self) -> PhysicalAddress {
        PhysicalAddress(self.start as usize)
    }

    pub fn end(&self) -> PhysicalAddress {
        PhysicalAddress(self.end as usize)
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub unsafe fn data<P>(&self, physical_memory: &P) -> &[u8]
        where P: PhysicalMemory
    {
        let length = self.end - self.start;
        let length = length as usize;

        let data = physical_memory.slice::<u8>(self.start(), length);

        data
    }
}

#[derive(Debug)]
pub struct BootDev {
    bios_dev: u32,
    partition: u32,
    sub_partition: u32,
}

impl BootDev {
    fn new(bios_dev: u32, partition: u32, sub_partition: u32) -> Self {
        Self {
            bios_dev,
            partition,
            sub_partition
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum MemoryMapEntryType {
    Available,
    Reserved,
    AcpiReclaimable,
    Nvs,
    Badram,
}

impl From<u32> for MemoryMapEntryType {
    fn from(value: u32) -> Self {
        return match value {
            0x01 => MemoryMapEntryType::Available,
            0x02 => MemoryMapEntryType::Reserved,
            0x03 => MemoryMapEntryType::AcpiReclaimable,
            0x04 => MemoryMapEntryType::Nvs,
            0x05 => MemoryMapEntryType::Badram,

            _ => panic!("Unknown value for Memory Map entry type"),
        }
    }
}

#[derive(Debug)]
pub struct MemoryMapEntry {
    addr: u64,
    length: u64,
    typ: MemoryMapEntryType,
}

impl MemoryMapEntry {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let addr = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let length = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let typ = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        let typ = MemoryMapEntryType::from(typ);

        Some(Self {
            addr,
            length,
            typ
        })
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn typ(&self) -> MemoryMapEntryType {
        self.typ
    }
}

pub struct MemoryMapIter<'a> {
    bytes: &'a [u8],
    entry_count: usize,
    entry_size: usize,
    current_index: usize
}

impl<'a> MemoryMapIter<'a> {
    fn new(bytes: &'a [u8], entry_count: usize, entry_size: usize) -> Self {
        Self {
            bytes,
            entry_count,
            entry_size,
            current_index: 0
        }
    }
}

impl<'a> Iterator for MemoryMapIter<'a> {
    type Item = MemoryMapEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.entry_count {
            return None;
        }

        let start = self.entry_size * self.current_index;
        let bytes = &self.bytes[start..start + self.entry_size];
        let entry = MemoryMapEntry::parse(bytes)?;

        self.current_index += 1;
        Some(entry)
    }
}

pub struct MemoryMap<'a> {
    bytes: &'a [u8],
    start_offset: usize,
    entry_count: usize,
    entry_size: usize,
}

impl<'a> MemoryMap<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }

        let tag_type = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        assert!(tag_type == 6, "Mismatch tag type");

        let tag_size = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let entry_size = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let _entry_version = u32::from_le_bytes(bytes[12..16].try_into().ok()?);

        let entry_count = tag_size.checked_sub(16)? / entry_size;

        let entry_size = entry_size as usize;
        let entry_count = entry_count as usize;

        Some(Self {
            bytes,
            start_offset: 16,
            entry_count,
            entry_size,
        })
    }

    pub fn iter(&self) -> MemoryMapIter {
        MemoryMapIter::new(&self.bytes[self.start_offset..],
                                       self.entry_count,
                                       self.entry_size)
    }
}

impl<'a> core::fmt::Debug for MemoryMap<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MultibootTagMemoryMap")
            .finish()
    }
}

#[derive(Debug)]
pub struct Framebuffer {
    addr: usize,
    pitch: u32,
    width: u32,
    height: u32,
    bits_per_pixel: u8,
}

#[allow(dead_code)]
impl Framebuffer {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let tag_type = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        assert!(tag_type == 8, "Mismatch tag type");
        let _tag_size = u32::from_le_bytes(bytes[4..8].try_into().ok()?);

        let addr = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let addr = addr as usize;

        let pitch = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
        let width = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
        let height = u32::from_le_bytes(bytes[24..28].try_into().ok()?);
        let bits_per_pixel = bytes[28];
        let _typ = bytes[29];

        Some(Self {
            addr,
            pitch,
            width,
            height,
            bits_per_pixel
        })
    }

    pub fn addr(&self) -> usize {
        self.addr
    }

    pub fn pitch(&self) -> u32 {
        self.pitch
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn bits_per_pixel(&self) -> u8 {
        self.bits_per_pixel
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ElfSectionType {
    Null,
    ProgramBits,
    SymbolTable,
    StringTable,
    RelocationEntriesAddends,
    HashTable,
    DynamicInfo,
    Note,
    NoBits,
    RelocationEntries,
    DynamicSymbolTable,
    InitArray,
    FiniArray,
    PreinitArray,
    Group,
    ExtendedSectionIndicies,
    OsSpecific(u32),
}

impl From<u32> for ElfSectionType {
    fn from(value: u32) -> Self {
        return match value {
            0x00 => ElfSectionType::Null,
            0x01 => ElfSectionType::ProgramBits,
            0x02 => ElfSectionType::SymbolTable,
            0x03 => ElfSectionType::StringTable,
            0x04 => ElfSectionType::RelocationEntriesAddends,
            0x05 => ElfSectionType::HashTable,
            0x06 => ElfSectionType::DynamicInfo,
            0x07 => ElfSectionType::Note,
            0x08 => ElfSectionType::NoBits,
            0x09 => ElfSectionType::RelocationEntries,
            // 0x0A - Reserved
            0x0B => ElfSectionType::DynamicSymbolTable,
            0x0E => ElfSectionType::InitArray,
            0x0F => ElfSectionType::FiniArray,
            0x10 => ElfSectionType::PreinitArray,
            0x11 => ElfSectionType::Group,
            0x12 => ElfSectionType::ExtendedSectionIndicies,

            _ => ElfSectionType::OsSpecific(value),
        }
    }
}

#[derive(Debug)]
pub struct ElfSection {
    name_index: u32,
    typ: ElfSectionType,
    flags: u64,
    addr: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    addr_align: u64,
    entry_size: u64,
}

#[allow(dead_code)]
impl ElfSection {
    fn parse(bytes: &[u8]) -> Option<Self> {
        assert!(bytes.len() >= 64, "ELF section mismatch length");

        let name_index = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        let typ = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let flags = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let addr = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
        let offset = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
        let size = u64::from_le_bytes(bytes[32..40].try_into().ok()?);
        let link = u32::from_le_bytes(bytes[40..44].try_into().ok()?);
        let info = u32::from_le_bytes(bytes[44..48].try_into().ok()?);
        let addr_align = u64::from_le_bytes(bytes[48..56].try_into().ok()?);
        let entry_size = u64::from_le_bytes(bytes[56..64].try_into().ok()?);

        let typ = ElfSectionType::from(typ);

        Some(Self {
            name_index,
            typ,
            flags,
            addr,
            offset,
            size,
            link,
            info,
            addr_align,
            entry_size,
        })
    }

    pub fn name_index(&self) -> u32 {
        self.name_index
    }

    pub fn typ(&self) -> ElfSectionType {
        self.typ
    }

    pub fn flags(&self) -> u64 {
        self.flags
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn link(&self) -> u32 {
        self.link
    }

    pub fn info(&self) -> u32 {
        self.info
    }

    pub fn addr_align(&self) -> u64 {
        self.addr_align
    }

    pub fn entry_size(&self) -> u64 {
        self.entry_size
    }
}

pub struct ElfStringTable<'a> {
    bytes: &'a [u8],
}

impl<'a> ElfStringTable<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
        }
    }

    pub fn string(&self, index: u32) -> Option<&'a str> {
        let index = index as usize;

        let length = {
            let mut length = 0;
            let mut offset = index;

            while self.bytes[offset] != 0 {
                offset += 1;
                length += 1;
            }

            length
        };

        core::str::from_utf8(&self.bytes[index..index + length]).ok()
    }
}

#[derive(Debug)]
pub struct ElfSections<'a> {
    bytes: &'a [u8],
    start_offset: usize,
    num_entries: u32,
    entry_size: u32,
    string_table_index: u32
}

impl<'a> ElfSections<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        let tag_type = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        assert!(tag_type == 9, "Mismatch tag type");
        let _tag_size = u32::from_le_bytes(bytes[4..8].try_into().ok()?);

        let num_entries = u32::from_le_bytes(bytes[8..12].try_into().ok()?);

        let entry_size = u32::from_le_bytes(bytes[12..16].try_into().ok()?);

        let string_table_index =
            u32::from_le_bytes(bytes[16..20].try_into().ok()?);

        Some(Self {
            bytes,
            start_offset: 20,
            num_entries,
            entry_size,
            string_table_index
        })
    }

    pub fn iter(&self) -> ElfSectionIter {
        ElfSectionIter::new(&self.bytes[self.start_offset..],
                            self.num_entries,
                            self.entry_size)
    }

    pub fn string_table<P: PhysicalMemory>(&self, physical_memory: &P)
        -> Option<ElfStringTable>
    {
        let section = self.iter().nth(self.string_table_index as usize)?;

        // TODO(patrik): We need to check if the `section.addr` is inside the
        // kernel text area or inside the lower memory region
        let bytes = unsafe {
            physical_memory.slice::<u8>(PhysicalAddress(section.addr as usize),
                                        section.size as usize)
        };

        Some(ElfStringTable::new(bytes))
    }
}

pub struct ElfSectionIter<'a> {
    bytes: &'a [u8],
    num_sections: u32,
    entry_size: u32,
    current_index: u32,
}

impl<'a> ElfSectionIter<'a> {
    fn new(bytes: &'a [u8], num_sections: u32, entry_size: u32) -> Self {
        Self {
            bytes,
            num_sections,
            entry_size,
            current_index: 0,
        }
    }
}

impl<'a> Iterator for ElfSectionIter<'a> {
    type Item = ElfSection;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.num_sections {
            return None;
        }

        let entry_size = self.entry_size as usize;
        let offset = self.current_index as usize * entry_size;
        let bytes = &self.bytes[offset..offset + entry_size];
        let section = ElfSection::parse(bytes)?;

        self.current_index += 1;
        Some(section)
    }
}

pub struct TagIter<'a> {
    bytes: &'a [u8],
    offset: usize
}

impl<'a> TagIter<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0
        }
    }
}

impl<'a> Iterator for TagIter<'a> {
    type Item = Tag<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let tag_type =
            u32::from_le_bytes(
                self.bytes[self.offset..self.offset + 4].try_into().ok()?);

        if tag_type == 0 {
            return None;
        }

        let tag_size =
            u32::from_le_bytes(
                self.bytes[self.offset + 4..self.offset + 8].try_into().ok()?)
            as usize;

        let tag = match tag_type {
            1 => {
                // MULTIBOOT_TAG_TYPE_CMDLINE

                let start = self.offset + 8;
                // NOTE(patrik): 8 bytes for the tag header and 1 byte for
                // the null terminator
                let len = (tag_size - 8 - 1) as usize;

                let cmd_line =
                    core::str::from_utf8(
                        &self.bytes[start..start + len]).ok()?;

                Tag::CommandLine(cmd_line)
            }

            2 => {
                // MULTIBOOT_TAG_TYPE_BOOT_LOADER_NAME

                let start = self.offset + 8;
                // NOTE(patrik): 8 bytes for the tag header and 1 byte for
                // the null terminator
                let len = (tag_size - 8 - 1) as usize;

                let cmd_line = core::str::from_utf8(
                    &self.bytes[start..start + len]).ok()?;

                Tag::BootloaderName(cmd_line)
            }

            3 => {
                // MULTIBOOT_TAG_TYPE_MODULE

                let start = self.offset + 8;

                let module_start = u32::from_le_bytes(
                    self.bytes[start..start + 4].try_into().ok()?);
                let module_end = u32::from_le_bytes(
                    self.bytes[start + 4..start + 8].try_into().ok()?);

                let len = (tag_size - 8 - 1 - 8) as usize;
                let start = start + 8;

                let name = core::str::from_utf8(
                    &self.bytes[start..start + len]).ok()?;

                Tag::Module(Module {
                    start: module_start,
                    end: module_end,
                    name
                })
            }

            4 => {
                // MULTIBOOT_TAG_TYPE_BASIC_MEMINFO

                let start = self.offset + 8;
                let mem_lower = u32::from_le_bytes(
                    self.bytes[start..start + 4].try_into().ok()?);
                let mem_upper = u32::from_le_bytes(
                    self.bytes[start + 4..start + 8].try_into().ok()?);

                Tag::BasicMemInfo(mem_lower, mem_upper)
            }

            5 => {
                // MULTIBOOT_TAG_TYPE_BOOTDEV

                let start = self.offset + 8;
                let bios_dev = u32::from_le_bytes(
                    self.bytes[start..start + 4].try_into().ok()?);
                let partition = u32::from_le_bytes(
                    self.bytes[start + 4..start + 8].try_into().ok()?);
                let sub_partition = u32::from_le_bytes(
                    self.bytes[start + 8..start + 12].try_into().ok()?);

                let boot_dev =
                    BootDev::new(bios_dev, partition, sub_partition);
                Tag::BootDev(boot_dev)
            }

            6 => {
                // MULTIBOOT_TAG_TYPE_MMAP

                let bytes = &self.bytes[self.offset..self.offset + tag_size];
                let memory_map = MemoryMap::parse(bytes)?;

                Tag::MemoryMap(memory_map)
            }

            8 => {
                // MULTIBOOT_TAG_TYPE_FRAMEBUFFER

                let bytes = &self.bytes[self.offset..self.offset + tag_size];
                let framebuffer = Framebuffer::parse(bytes)?;

                Tag::Framebuffer(framebuffer)
            }

            9 => {
                // MULTIBOOT_TAG_TYPE_ELF_SECTIONS

                let bytes = &self.bytes[self.offset..self.offset + tag_size];
                let elf_sections = ElfSections::parse(bytes)?;

                Tag::ElfSections(elf_sections)
            }

            14 => {
                // MULTIBOOT_TAG_TYPE_ACPI_OLD

                let start = self.offset + 8;
                let sig = &self.bytes[start..start + 8];
                assert!(sig == b"RSD PTR ", "Wrong ACPI signature");

                // TODO(patrik): Check the checksum

                let _checksum = self.bytes[start + 8];
                let _oem_id = &self.bytes[start + 9..start + 14];
                let revision = self.bytes[start + 15];
                assert!(revision == 0,
                        "Revision should be 0 when ACPI 1.0 is used");

                let addr = u32::from_le_bytes(
                    self.bytes[start + 16..start + 20].try_into().ok()?);

                let addr = PhysicalAddress(addr as usize);
                Tag::Acpi1(addr)
            }

            15 => {
                // MULTIBOOT_TAG_TYPE_ACPI_NEW

                let start = self.offset + 8;
                let sig = &self.bytes[start..start + 8];
                assert!(sig == b"RSD PTR ", "Wrong ACPI signature");

                // TODO(patrik): Check the checksum

                let _checksum = self.bytes[start + 8];
                let _oem_id = &self.bytes[start + 9..start + 14];
                let revision = self.bytes[start + 15];
                assert!(revision == 2,
                        "Revision should be 0 when ACPI 1.0 is used");

                let _rsdt_addr = u32::from_le_bytes(
                    self.bytes[start + 16..start + 20].try_into().ok()?);

                let length = u32::from_le_bytes(
                    self.bytes[start + 20..start + 24].try_into().ok()?);

                let xsdt_addr = u64::from_le_bytes(
                    self.bytes[start + 24..start + 28].try_into().ok()?);

                let _exteneded_checksum = u8::from_le_bytes(
                    self.bytes[start + 28..start + 29].try_into().ok()?);

                let addr = PhysicalAddress(xsdt_addr as usize);
                let length = length as usize;
                Tag::Acpi2(addr, length)
            }

            21 => {
                // MULTIBOOT_TAG_TYPE_LOAD_BASE_ADDR

                let start = self.offset + 8;
                let addr = u32::from_le_bytes(
                    self.bytes[start..start + 4].try_into().ok()?);

                let addr = addr as usize;
                Tag::LoadBaseAddr(addr)
            }

            _ => Tag::Unknown(tag_type),
        };

        self.offset += ((tag_size + 7) & !7) as usize;
        Some(tag)
    }
}

pub struct Multiboot<'a> {
    bytes: &'a [u8],
    start_offset: usize,
}

impl<'a> Multiboot<'a> {
    pub unsafe fn from_addr<P: PhysicalMemory>(physical_memory: &P,
                                               structure_addr: PhysicalAddress)
        -> Self
    {
        let total_size =
            physical_memory.read::<u32>(structure_addr);

        let bytes =
            physical_memory.slice::<u8>(structure_addr, total_size as usize);

        Self {
            bytes,
            start_offset: 8,
        }
    }

    pub fn tags(&self) -> TagIter {
        TagIter::new(&self.bytes[self.start_offset..])
    }

    pub fn find_memory_map(&self) -> Option<MemoryMap> {
        for tag in self.tags() {
            if let Tag::MemoryMap(result) = tag {
                return Some(result);
            }
        }

        None
    }

    pub fn find_command_line(&self) -> Option<String> {
        for tag in self.tags() {
            if let Tag::CommandLine(result) = tag {
                return Some(result.to_string());
            }
        }

        None
    }

    pub fn modules<F>(&self, callback: F)
        where F: Fn(Module)
    {
        for tag in self.tags() {
            if let Tag::Module(m) = tag {
                callback(m);
            }
        }
    }
}
