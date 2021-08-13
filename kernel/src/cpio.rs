//! Module to handle and read from a CPIO archive file

use alloc::string::String;

#[derive(Copy, Clone, Debug)]
#[repr(C, packed)]
struct CPIOHeader {
    magic: u16,
    dev: u16,
    ino: u16,
    mode: u16,
    uid: u16,
    gid: u16,
    nlink: u16,
    rdev: u16,
    mtime: u32,
    namesize: u16,
    filesize: [u16; 2],
}

pub struct CPIO<'a> {
    data: &'a [u8],
}

impl<'a> CPIO<'a> {
    pub fn binary(data: &'a [u8]) -> Self {
        Self {
            data
        }
    }

    pub unsafe fn read_file(&self, path: String) -> Option<&[u8]> {
        let mut start = 0;
        loop {
            let header_ptr = self.data[start..].as_ptr() as *const CPIOHeader;
            let header = core::ptr::read(header_ptr);
            start += core::mem::size_of::<CPIOHeader>();

            let filesize =
                (header.filesize[0] as u32) << 16 | (header.filesize[1] as u32);
            let filesize = filesize as usize;
            // NOTE(patrik): -1 for the null byte
            let namesize = header.namesize as usize - 1;

            let name =
                core::str::from_utf8(&self.data[start..start+namesize])
                    .expect("Failed to convert the name for the file");

            if header.namesize % 2 == 0 {
                // Namesize is even
                start += namesize + 1;
            } else {
                // Namesize is odd
                // NOTE(patrik): if the namesize is odd then their is a extra
                // null byte in the name that we need to skip over
                start += namesize + 2;
            }

            if name == path {
                return Some(&self.data[start..start + filesize]);
            }

            if name == "TRAILER!!!" {
                return None;
            }

            if filesize % 2 == 0 {
                start += filesize;
            } else {
                start += filesize + 1;
            }
        }
    }
}
