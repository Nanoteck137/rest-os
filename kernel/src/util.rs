//! Module for some utility structs and functions

use spin::{ Mutex, MutexGuard };
use core::sync::atomic::{ AtomicUsize, Ordering };

/// Wrapper around the spin::Mutex used for the global allocator
pub struct Locked<A> {
    inner: Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> MutexGuard<A> {
        self.inner.lock()
    }
}

pub struct AutoAtomicRef(AtomicUsize);

impl AutoAtomicRef {
    pub const fn new(initial_value: usize) -> Self {
        Self(AtomicUsize::new(initial_value))
    }

    pub fn increment(&self) -> AutoAtomicRefGuard {
        let count = self.0.fetch_add(1, Ordering::SeqCst);
        count.checked_add(1)
            .expect("Integer overflow for AutoAtomicRef::increment");

        AutoAtomicRefGuard(self)
    }

    pub fn count(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

pub struct AutoAtomicRefGuard<'a>(&'a AutoAtomicRef);

impl<'a> Drop for AutoAtomicRefGuard<'a> {
    fn drop(&mut self) {
        let count = (self.0).0.fetch_sub(1, Ordering::SeqCst);
        count.checked_sub(1)
            .expect("Integer underflow for AutoAtomicRefGuard::drop");
    }
}

pub fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

pub fn align_down(value: usize, align: usize) -> usize {
    value & !(align - 1)
}
