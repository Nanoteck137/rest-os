//! Module to parse and retrive infomation from a Elf file

#![no_std]
#![allow(dead_code)]

#[macro_use]
extern crate bitflags;

use core::convert::TryInto;
use core::convert::TryFrom;

#[derive(Debug)]
pub enum Error {
    InvalidByteBuffer,
    InvalidMagic,
    InvalidIdentClass,
    InvalidIdentData,
    InvalidIdentOsAbi,
    InvalidElfType,
    InvalidMachine(u16),
    FailedToParseHeader,
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Copy, Clone, PartialEq, Debug)]
enum IdentClass {
    Class32,
    Class64,
}

impl TryFrom<u8> for IdentClass {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Class32),
            2 => Ok(Self::Class64),
            _ => Err(Error::InvalidIdentClass)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum IdentData {
    LittleEndian,
    BigEndian,
}

impl TryFrom<u8> for IdentData {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::LittleEndian),
            2 => Ok(Self::BigEndian),

            _ => Err(Error::InvalidIdentData),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum IdentOsAbi {
    SystemV,
    HPUX,
    NetBSD,
    Linux,
    GNUHurd,
    Solaris,
    AIX,
    IRIX,
    FreeBSD,
    Tru64,
    NovellModesto,
    OpenBSD,
    OpenVMS,
    NonStopKernel,
    AROS,
    FenixOS,
    CloudABI,
    OpenVOS,
}

impl TryFrom<u8> for IdentOsAbi {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x00 => Ok(Self::SystemV),
            0x01 => Ok(Self::HPUX),
            0x02 => Ok(Self::NetBSD),
            0x03 => Ok(Self::Linux),
            0x04 => Ok(Self::GNUHurd),

            0x06 => Ok(Self::Solaris),
            0x07 => Ok(Self::AIX),
            0x08 => Ok(Self::IRIX),
            0x09 => Ok(Self::FreeBSD),
            0x0a => Ok(Self::Tru64),
            0x0b => Ok(Self::NovellModesto),
            0x0c => Ok(Self::OpenBSD),
            0x0d => Ok(Self::OpenVMS),
            0x0e => Ok(Self::NonStopKernel),
            0x0f => Ok(Self::AROS),
            0x10 => Ok(Self::FenixOS),
            0x11 => Ok(Self::CloudABI),
            0x12 => Ok(Self::OpenVOS),

            _ => Err(Error::InvalidIdentOsAbi),
        }
    }
}

#[derive(Debug)]
struct Ident {
    class: IdentClass,
    data: IdentData,
    version: u8,
    os_abi: IdentOsAbi,
    abi_version: u8,
}

impl Ident {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            return Err(Error::InvalidByteBuffer);
        }

        let class = IdentClass::try_from(bytes[4])?;
        let data = IdentData::try_from(bytes[5])?;

        let version = bytes[6];
        let os_abi = IdentOsAbi::try_from(bytes[7])?;
        let abi_version = bytes[8];

        let _pad = &bytes[9..16];

        Ok(Self {
            class,
            data,
            version,
            os_abi,
            abi_version,
        })
    }

    fn class(&self) -> IdentClass {
        self.class
    }

    fn data(&self) -> IdentData {
        self.data
    }

    fn version(&self) -> u8 {
        self.version
    }

    fn os_abi(&self) -> IdentOsAbi {
        self.os_abi
    }

    fn abi_version(&self) -> u8 {
        self.abi_version
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum ElfType {
    None,
    Rel,
    Exec,
    Dyn,
    Core,

    Os(u8),
    Proc(u8)
}

impl TryFrom<u16> for ElfType {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            0x00 => Ok(Self::None),
            0x01 => Ok(Self::Rel),
            0x02 => Ok(Self::Exec),
            0x03 => Ok(Self::Dyn),
            0x04 => Ok(Self::Core),

            0xFE00..=0xFEFF => Ok(Self::Os((value & 0xff) as u8)),
            0xFF00..=0xFFFF => Ok(Self::Proc((value & 0xff) as u8)),

            _ => Err(Error::InvalidElfType)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Machine {
    AMD64,
}

impl TryFrom<u16> for Machine {
    type Error = Error;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            0x3e => Ok(Self::AMD64),

            _ => Err(Error::InvalidMachine(value))
        }
    }
}

#[derive(Debug)]
pub struct Elf<'a> {
    bytes: &'a [u8],

    ident: Ident,
    typ: ElfType,
    machine: Machine,

    entry: u64,
    flags: u32,

    program_table_offset: u64,
    program_table_entry_size: usize,
    num_program_table_entries: usize,

    section_table_offset: u64,
    section_table_entry_size: usize,
    num_section_table_entries: usize,

    string_section_index: u16,
}

impl<'a> Elf<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < 16 {
            return Err(Error::InvalidByteBuffer);
        }

        if &bytes[0..4] != b"\x7fELF" {
            return Err(Error::InvalidMagic);
        }

        let ident = Ident::parse(&bytes[0..16])?;

        assert!(ident.class() == IdentClass::Class64);
        assert!(ident.data() == IdentData::LittleEndian);
        assert!(ident.os_abi() == IdentOsAbi::SystemV);

        let map_err = |_| Error::FailedToParseHeader;

        let typ = u16::from_le_bytes(
            bytes[16..18].try_into().map_err(map_err)?);
        let typ = ElfType::try_from(typ)?;

        let machine = u16::from_le_bytes(
            bytes[18..20].try_into().map_err(map_err)?);
        let machine = Machine::try_from(machine)?;

        let _version2 = u32::from_le_bytes(
            bytes[20..24].try_into().map_err(map_err)?);

        let entry = u64::from_le_bytes(
            bytes[24..32].try_into().map_err(map_err)?);

        let program_table_offset =
            u64::from_le_bytes(bytes[32..40].try_into().map_err(map_err)?);

        let section_table_offset =
            u64::from_le_bytes(bytes[40..48].try_into().map_err(map_err)?);

        let flags = u32::from_le_bytes(
            bytes[48..52].try_into().map_err(map_err)?);

        let ehsize = u16::from_le_bytes(
            bytes[52..54].try_into().map_err(map_err)?);

        assert!(ehsize == 64);

        let program_table_entry_size =
            u16::from_le_bytes(bytes[54..56].try_into().map_err(map_err)?);
        let program_table_entry_size = program_table_entry_size as usize;

        let num_program_table_entries =
            u16::from_le_bytes(bytes[56..58].try_into().map_err(map_err)?);
        let num_program_table_entries = num_program_table_entries as usize;

        let section_table_entry_size =
            u16::from_le_bytes(bytes[58..60].try_into().map_err(map_err)?);
        let section_table_entry_size = section_table_entry_size as usize;

        let num_section_table_entries =
            u16::from_le_bytes(bytes[60..62].try_into().map_err(map_err)?);
        let num_section_table_entries = num_section_table_entries as usize;

        let string_section_index =
            u16::from_le_bytes(bytes[62..64].try_into().map_err(map_err)?);

        Ok(Elf {
            bytes,

            ident,
            typ,
            machine,

            entry,
            flags,

            program_table_offset,
            program_table_entry_size,
            num_program_table_entries,

            section_table_offset,
            section_table_entry_size,
            num_section_table_entries,

            string_section_index,
        })
    }

    pub fn program_headers(&self) -> ProgramHeaderIter {
        ProgramHeaderIter::new(self.bytes,
                               self.program_table_offset as usize,
                               self.program_table_entry_size,
                               self.num_program_table_entries)
    }

    pub fn program_data(&self, program_header: &ProgramHeader) -> &'a [u8] {
        let size = program_header.file_size() as usize;
        let start = program_header.offset() as usize;
        let end = start + size;

        &self.bytes[start..end]
    }

    pub fn entry(&self) -> u64 {
        self.entry
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum ProgramHeaderType {
    Null,
    Load,
    Dynamic,
    Interp,
    Note,
    Shlib,
    ProgramHeader,
    ThreadLocalStorage,
    Os(u32),
    Proc(u32),
}

impl TryFrom<u32> for ProgramHeaderType {
    type Error = ();

    fn try_from(value: u32) -> core::result::Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Null),
            0x01 => Ok(Self::Load),
            0x02 => Ok(Self::Dynamic),
            0x03 => Ok(Self::Interp),
            0x04 => Ok(Self::Note),
            0x05 => Ok(Self::Shlib),
            0x06 => Ok(Self::ProgramHeader),
            0x07 => Ok(Self::ThreadLocalStorage),

            0x60000000..=0x6FFFFFFF => Ok(Self::Os(value)),
            0x70000000..=0x7FFFFFFF => Ok(Self::Proc(value)),

            _ => Err(()),
        }
    }
}

bitflags! {
    pub struct ProgramHeaderFlags: u32 {
        const EXECUTE = 0x1;
        const WRITE   = 0x2;
        const READ    = 0x4;
    }
}

#[derive(Debug)]
pub struct ProgramHeader {
    typ: ProgramHeaderType,
    flags: ProgramHeaderFlags,
    offset: u64,
    vaddr: u64,
    paddr: u64,
    file_size: u64,
    memory_size: u64,
    alignment: u64,
}

impl ProgramHeader {
    fn parse(bytes: &[u8]) -> Option<Self> {
        // TODO(patrik): Check bytes for correct length

        let typ = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        let typ = ProgramHeaderType::try_from(typ).ok()?;

        let flags = u32::from_le_bytes(bytes[4..8].try_into().ok()?);

        let offset = u64::from_le_bytes(bytes[8..16].try_into().ok()?);

        let vaddr = u64::from_le_bytes(bytes[16..24].try_into().ok()?);

        let paddr = u64::from_le_bytes(bytes[24..32].try_into().ok()?);

        let file_size = u64::from_le_bytes(bytes[32..40].try_into().ok()?);

        let memory_size = u64::from_le_bytes(bytes[40..48].try_into().ok()?);

        let alignment = u64::from_le_bytes(bytes[48..56].try_into().ok()?);

        let flags = ProgramHeaderFlags::from_bits_truncate(flags);

        Some(Self {
            typ,
            flags,
            offset,
            vaddr,
            paddr,
            file_size,
            memory_size,
            alignment
        })
    }

    pub fn typ(&self) -> ProgramHeaderType {
        self.typ
    }

    pub fn flags(&self) -> ProgramHeaderFlags {
        self.flags
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn vaddr(&self) -> u64 {
        self.vaddr
    }

    pub fn paddr(&self) -> u64 {
        self.paddr
    }

    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    pub fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub fn alignment(&self) -> u64 {
        self.alignment
    }
}

pub struct ProgramHeaderIter<'a> {
    bytes: &'a [u8],

    offset: usize,
    entry_size: usize,
    max_entries: usize,
    index: usize,
}

impl<'a> ProgramHeaderIter<'a> {
    fn new(bytes: &'a [u8], offset: usize,
           entry_size: usize, max_entries: usize)
        -> Self
    {
        Self {
            bytes,

            offset,
            entry_size,
            max_entries,
            index: 0,
        }
    }
}

impl<'a> Iterator for ProgramHeaderIter<'a> {
    type Item = ProgramHeader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.max_entries {
            return None;
        }

        let start = self.offset + self.entry_size * self.index;
        let end = start + self.entry_size;

        let result = ProgramHeader::parse(&self.bytes[start..end])?;

        self.index += 1;

        Some(result)
    }
}

