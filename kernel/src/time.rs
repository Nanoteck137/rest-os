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

unsafe fn calibrate() {
    println!("Calibrating the TSC clock");

    let start = x86_64::rdtsc();
    TSC_START.store(start, Ordering::Relaxed);

    let start = x86_64::rdtsc();

    x86_64::out8(0x43, 0x30);
    x86_64::out8(0x40, 0xff);
    x86_64::out8(0x40, 0xff);

    loop {
        x86_64::out8(0x43, 0xe2);

        if (x86_64::in8(0x40) & 0x80) != 0 {
            break;
        }
    }

    let end = x86_64::rdtsc();

    let elapsed = 65535f64 / 1193182f64;

    let computed_rate = ((end - start) as f64) / elapsed / 1000000.0;
    let rounded_rate = (((computed_rate / 100.0) + 0.5) as u64) * 100;

    let old_freq = TSC_FREQ_MHZ.load(Ordering::Relaxed);
    println!("Old TSC freq: {} MHz", old_freq);
    println!("New TSC freq: {} MHz", rounded_rate);

    TSC_FREQ_MHZ.store(rounded_rate, Ordering::Relaxed);
}

pub fn initialize() {
    unsafe {
        calibrate();
    }
}
