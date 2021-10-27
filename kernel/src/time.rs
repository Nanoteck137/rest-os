//! Module to handle simple time managment
//! Reference: https://github.com/gamozolabs/chocolate_milk/blob/master/kernel/src/time.rs

use crate::arch::x86_64;

use core::sync::atomic::{ AtomicU64, Ordering };

static TSC_FREQ_MHZ: AtomicU64 = AtomicU64::new(3000);
static TSC_START: AtomicU64 = AtomicU64::new(0);

#[inline]
pub fn tsc_freq_mhz() -> u64 {
    TSC_FREQ_MHZ.load(Ordering::Relaxed)
}

#[inline]
pub fn uptime() -> f64 {
    let start = TSC_START.load(Ordering::Relaxed);
    return if start == 0 {
        0.0
    } else {
        elapsed(start)
    };
}

#[inline]
pub fn future(microseconds: u64) -> u64 {
    x86_64::rdtsc() + (microseconds * tsc_freq_mhz())
}

#[inline]
pub fn elapsed(start_time: u64) -> f64 {
    (x86_64::rdtsc() - start_time) as f64 /
        tsc_freq_mhz() as f64 / 1000000.0
}

pub fn sleep(microseconds: u64) {
    let wait = future(microseconds);

    while x86_64::rdtsc() < wait {
        core::hint::spin_loop();
    }
}

fn calibrate() {
    let start = x86_64::rdtsc();
    TSC_START.store(start, Ordering::Relaxed);
}

pub fn initialize() {
    calibrate();
}
