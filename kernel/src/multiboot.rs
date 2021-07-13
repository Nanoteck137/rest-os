use core::convert::TryInto;
use super::println;

pub struct Multiboot<'a> {
    bytes: &'a [u8],
    start_offset: usize,
}

impl<'a> Multiboot<'a> {
    pub unsafe fn from_addr(structure_addr: usize) -> Self {
        let total_size =
            core::ptr::read_volatile(structure_addr as *const u32);

        let ptr = structure_addr as *const u8;
        let bytes = core::slice::from_raw_parts(ptr, total_size as usize);

        Self {
            bytes,
            start_offset: 8,
        }
    }

    pub fn tags(&self) -> MultibootTagIter {
        MultibootTagIter::new(&self.bytes[self.start_offset..])
    }
}

pub struct MultibootTagIter<'a> {
    bytes: &'a [u8],
    offset: usize
}

impl<'a> MultibootTagIter<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0
        }
    }
}

pub struct MultibootTagMemoryMap<'a> {
    bytes: &'a [u8],
    start_offset: usize,
    entry_count: usize,
    entry_size: usize,
}

impl<'a> core::fmt::Debug for MultibootTagMemoryMap<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MultibootTagMemoryMap")
            .finish()
    }
}

impl<'a> MultibootTagMemoryMap<'a> {
    fn parse(bytes: &'a [u8]) -> Option<Self> {
        if bytes.len() < 16 {
            return None;
        }

        let tag_type = u32::from_le_bytes(bytes[0..4].try_into().ok()?);
        assert!(tag_type == 6, "Mismatch tag type");

        let tag_size = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        let entry_size = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let _entry_version = u32::from_le_bytes(bytes[12..16].try_into().ok()?);

        let entry_count = (tag_size - 16) / entry_size;

        let entry_size = entry_size as usize;
        let entry_count = entry_count as usize;

        Some(Self {
            bytes,
            start_offset: 16,
            entry_count,
            entry_size,
        })
    }

    pub fn iter(&self) -> MultibootTagMemoryMapIter {
        MultibootTagMemoryMapIter::new(&self.bytes[self.start_offset..],
                                       self.entry_count,
                                       self.entry_size)
    }
}

#[derive(Debug)]
pub struct MultibootTagMemoryEntry {
    addr: u64,
    length: u64,
    typ: u32,
}

impl MultibootTagMemoryEntry {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let addr = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let length = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
        let typ = u32::from_le_bytes(bytes[16..20].try_into().ok()?);


        Some(Self {
            addr,
            length,
            typ
        })
    }
}

pub struct MultibootTagMemoryMapIter<'a> {
    bytes: &'a [u8],
    entry_count: usize,
    entry_size: usize,
    current_index: usize
}

impl<'a> MultibootTagMemoryMapIter<'a> {
    fn new(bytes: &'a [u8], entry_count: usize, entry_size: usize) -> Self {
        Self {
            bytes,
            entry_count,
            entry_size,
            current_index: 0
        }
    }
}

impl<'a> Iterator for MultibootTagMemoryMapIter<'a> {
    type Item = MultibootTagMemoryEntry;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index >= self.entry_count {
            return None;
        }

        let start = self.entry_size * self.current_index;
        let bytes = &self.bytes[start..start + self.entry_size];
        let entry = MultibootTagMemoryEntry::parse(bytes)?;

        self.current_index += 1;
        Some(entry)
    }
}

#[derive(Debug)]
pub struct MultibootTagFramebuffer {
    addr: usize,
    pitch: u32,
    width: u32,
    height: u32,
    bits_per_pixel: u8,
}

impl MultibootTagFramebuffer {
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
}

#[derive(Debug)]
pub enum MultibootTag<'a> {
    CommandLine(&'a str),
    BootloaderName(&'a str),
    MemoryMap(MultibootTagMemoryMap<'a>),
    Framebuffer(MultibootTagFramebuffer),
    Unknown(u32),
}

impl<'a> Iterator for MultibootTagIter<'a> {
    type Item = MultibootTag<'a>;

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

                MultibootTag::CommandLine(cmd_line)
            }

            2 => {
                // MULTIBOOT_TAG_TYPE_BOOT_LOADER_NAME

                let start = self.offset + 8;
                // NOTE(patrik): 8 bytes for the tag header and 1 byte for
                // the null terminator
                let len = (tag_size - 8 - 1) as usize;

                let cmd_line =
                    core::str::from_utf8(
                        &self.bytes[start..start + len]).ok()?;

                MultibootTag::BootloaderName(cmd_line)
            }

            6 => {
                // MULTIBOOT_TAG_TYPE_MMAP

                let bytes = &self.bytes[self.offset..self.offset + tag_size];
                let memory_map = MultibootTagMemoryMap::parse(bytes)?;

                MultibootTag::MemoryMap(memory_map)
            }

            8 => {
                // MULTIBOOT_TAG_TYPE_FRAMEBUFFER

                let bytes = &self.bytes[self.offset..self.offset + tag_size];
                let framebuffer = MultibootTagFramebuffer::parse(bytes)?;

                MultibootTag::Framebuffer(framebuffer)
            }

            _ => MultibootTag::Unknown(tag_type),
        };

        self.offset += ((tag_size + 7) & !7) as usize;
        /*if self.offset >= self.total_size {
            return None;
        }*/

        Some(tag)
    }
}
