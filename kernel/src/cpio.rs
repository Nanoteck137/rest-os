//! Module to handle and read from a CPIO archive file
//! Reference: https://www.systutorials.com/docs/linux/man/5-cpio/

use crate::mm::VirtualAddress;
use crate::util::align_up;

use alloc::string::String;
use alloc::vec::Vec;

#[derive(Copy, Clone, Debug)]
pub enum CPIOKind {
    Binary,
    Odc,
    Newc,
    Crc
}

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct CPIOInfo {
    name_size: usize,
    file_size: usize,
}

impl CPIOInfo {
    fn parse(bytes: &[u8], kind: CPIOKind) -> Option<(CPIOInfo, usize)> {
        return match kind {
            CPIOKind::Binary => {
                if bytes.len() < 26 {
                    return None;
                }

                unimplemented!();

                // NOTE(patrik): This assumes that the CPIOHeader structure
                // is the same as the cpio binary format header
                // let ptr = bytes.as_ptr() as *const CPIOHeader;
                // let result = unsafe { core::ptr::read(ptr) };

                // Some((result, 26))
            },

            CPIOKind::Odc => {
                unimplemented!();
            },

            CPIOKind::Newc => {
                if bytes.len() < 110 {
                    return None;
                }

                let parse_u16 = |data| {
                    let s = core::str::from_utf8(data).ok()?;
                    u32::from_str_radix(s, 8).ok()
                };

                let parse_u32 = |data| {
                    let s = core::str::from_utf8(data).ok()?;
                    u32::from_str_radix(s, 16).ok()
                };

                let magic = parse_u16(&bytes[0..6])?;
                let ino = parse_u32(&bytes[6..14])?;
                let mode = parse_u32(&bytes[14..22])?;
                let uid = parse_u32(&bytes[22..30])?;
                let gid = parse_u32(&bytes[30..38])?;
                let nlink = parse_u32(&bytes[38..46])?;
                let mtime = parse_u32(&bytes[46..54])?;
                let filesize = parse_u32(&bytes[54..62])?;
                let devmajor = parse_u32(&bytes[62..70])?;
                let devminor = parse_u32(&bytes[70..78])?;
                let rdevmajor = parse_u32(&bytes[78..86])?;
                let rdevminor = parse_u32(&bytes[86..94])?;
                let namesize = parse_u32(&bytes[94..102])?;
                let check = parse_u32(&bytes[102..110])?;

                Some((CPIOInfo {
                    file_size: filesize as usize,
                    name_size: namesize as usize,
                }, 110))
            },

            CPIOKind::Crc => {
                unimplemented!();
            },
        };
    }
}

fn pad_to_4(len: usize) -> usize {
    return match len % 4 {
        0 => 0,
        x => 4 - x,
    };
}

pub struct CPIO<'a> {
    data: &'a [u8],
    kind: CPIOKind,
}

impl<'a> CPIO<'a> {
    pub fn new(data_addr: VirtualAddress, data_size: usize, kind: CPIOKind)
        -> Self
    {
        let data = unsafe {
            core::slice::from_raw_parts(data_addr.0 as *const u8, data_size)
        };

        Self {
            data,
            kind
        }
    }

    pub unsafe fn read_file(&self, path: String) -> Option<&[u8]> {
        let mut start = 0;
        loop {
            let data = &self.data[start..];
            let (info, header_size) = CPIOInfo::parse(data, self.kind)?;
            start += header_size;

            /*
            let header_ptr = .as_ptr() as *const CPIOHeader;
            let header = core::ptr::read(header_ptr);
            start += core::mem::size_of::<CPIOHeader>();
            */

            let file_size = info.file_size;
            // NOTE(patrik): -1 for the null byte
            let name_size = info.name_size;

            let name_bytes = &self.data[start..start + name_size];
            let name =
                core::str::from_utf8(&name_bytes[..name_size - 1])
                    .expect("Failed to convert the name for the file");

            start += name_size;
            start += pad_to_4(header_size + name_size);

            // println!("Name: {} Offset: {:#x}", info.name_size, start);
            // println!("Name: {} Offset: {:#x}", off, start);

            /*
            if name_size % 2 == 0 {
                // Namesize is even
                start += name_size + 1;
            } else {
                // Namesize is odd
                // NOTE(patrik): if the namesize is odd then their is a extra
                // null byte in the name that we need to skip over
                start += name_size + 2;
            }
            */

            if name == path {
                println!("Test: {:x?}", &self.data[start..start+3]);
                return Some(&self.data[start..start + file_size]);
            }

            if name == "TRAILER!!!" {
                return None;
            }

            start += file_size;
            start += pad_to_4(file_size);

            /*
            if file_size % 2 == 0 {
                start += file_size;
            } else {
                start += file_size + 1;
            }
            */
        }
    }
}
