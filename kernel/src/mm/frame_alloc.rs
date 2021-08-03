//! This is the page frame allocator used by the kernel (page tables)

use super::{ PAGE_SIZE, Frame, PhysicalAddress };
use crate::multiboot::{ MemoryMap, MemoryMapEntryType };
use crate::util::align_down;

use core::convert::TryFrom;
use alloc::vec::Vec;

pub trait FrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame>;
    fn free_frame(&mut self, frame: Frame);
}

struct Bitmap {
    storage: Vec<u8>,
}

impl Bitmap {
    fn new(length: usize) -> Self {
        Self {
            storage: vec![0; length],
        }
    }

    fn len(&self) -> usize {
        self.storage.len() * 8
    }

    fn set_index(&mut self, index: usize, value: bool) {
        let byte_index = index / 8;
        let bit_index = index % 8;

        self.storage[byte_index] &= !(1 << bit_index);
        if value {
            self.storage[byte_index] |= 1 << bit_index;
        }
    }

    fn index(&self, index: usize) -> bool {
        let byte_index = index / 8;
        let bit_index = index % 8;

        if (self.storage[byte_index] & (1 << bit_index)) > 0 {
            return true;
        } else {
            return false;
        }
    }
}

struct BitmapRegion {
    start: PhysicalAddress,
    num_frames: usize,

    bitmap: Bitmap,
}

impl BitmapRegion {
    fn new(start: PhysicalAddress, num_frames: usize) -> Self {
        assert!(start.0 % PAGE_SIZE == 0,
                "BitmapRegion::new: ´start´ needs to be Page aligned");
        assert!(num_frames > 0, "Region cannot have 0 frames");

        let bitmap_size = num_frames / 8 + 1;
        Self {
            start,
            num_frames,

            bitmap: Bitmap::new(bitmap_size),
        }
    }

    fn start_addr(&self) -> PhysicalAddress {
        self.start
    }

    fn end_addr(&self) -> PhysicalAddress {
        PhysicalAddress(self.start.0 + (self.num_frames * 4096) - 1)
    }

    fn lock_frames(&mut self, start: PhysicalAddress, num_frames: usize) {
        assert!(start.0 % PAGE_SIZE == 0,
                "BitmapRegion::lock_frames: ´start´ needs to be Page aligned");
        let start = start.0 - self.start.0;
        let start = start / 4096;
        let end = start + num_frames;

        for i in start..end {
            self.bitmap.set_index(i, true);
        }
    }

    fn alloc_frame(&mut self) -> Option<Frame> {
        for i in 0..self.num_frames {
            if !self.bitmap.index(i) {
                self.bitmap.set_index(i, true);

                let addr = PhysicalAddress(i * 4096 + self.start.0);
                if addr.0 == 0x105000 {
                    panic!("Woot");
                }
                let frame = Frame::try_from(addr)
                    .expect("Failed to convert to frame");
                return Some(frame);
            }
        }

        None
    }

    fn free_frame(&mut self, frame: Frame) {
        let addr = PhysicalAddress::from(frame);
        assert!(addr >= self.start_addr() && addr <= self.end_addr());

        let addr = addr.0 - self.start.0;
        let index = addr / PAGE_SIZE;

        self.bitmap.set_index(index, false);
    }
}

impl core::fmt::Debug for BitmapRegion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let end = self.end_addr();
        let bitmap_length = self.bitmap.len() / 8;
        f.debug_struct("BitmapRegion")
            .field("start", &self.start)
            .field("end", &end)
            .field("num_frames", &self.num_frames)
            .field("bitmap_length", &bitmap_length)
            .finish()
    }
}

#[derive(Debug)]
pub struct BitmapFrameAllocator {
    bitmap_regions: Vec<BitmapRegion>,
}

impl BitmapFrameAllocator {
    pub fn new() -> Self {
        Self {
            bitmap_regions: Vec::new(),
        }
    }

    pub unsafe fn init(&mut self, memory_map: MemoryMap) -> Option<()> {
        for mmap_entry in memory_map.iter() {
            if mmap_entry.typ() == MemoryMapEntryType::Available {
                let addr = mmap_entry.addr() as usize;
                let length = mmap_entry.length() as usize;
                let new_length = align_down(length, 4096);

                let num_frames = new_length / 4096;

                let bitmap_region =
                    BitmapRegion::new(PhysicalAddress(addr), num_frames);
                self.bitmap_regions.push(bitmap_region);
            }
        }

        Some(())
    }

    pub fn lock_region(&mut self, addr: PhysicalAddress, length: usize)
        -> Option<()>
    {
        let start = addr;
        let end = PhysicalAddress(addr.0 + length - 1);
        for region in self.bitmap_regions.iter_mut() {
            if start >= region.start_addr() && end <= region.end_addr() {
                let num_frames = length / 4096;
                println!("Locking region: {:?} -> {}", addr, num_frames);
                region.lock_frames(start, num_frames);

                return Some(());
            }
        }

        None
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn alloc_frame(&mut self) -> Option<Frame> {
        for region in self.bitmap_regions.iter_mut() {
            if let Some(frame) = region.alloc_frame() {
                return Some(frame);
            }
        }

        return None;
    }

    fn free_frame(&mut self, frame: Frame) {
        let addr = PhysicalAddress::from(frame);
        for region in self.bitmap_regions.iter_mut() {
            if addr >= region.start_addr() && addr <= region.end_addr() {
                region.free_frame(frame);
                return;
            }
        }
    }
}
