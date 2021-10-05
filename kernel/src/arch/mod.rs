//! This module should be a interface for common architecture interfaces

pub mod x86_64;

pub use x86_64::ArchInfo;

pub fn early_initialize() {
    x86_64::early_initialize();
}

pub fn initialize() {
    x86_64::initialize();
}

pub fn debug_print_fmt(args: core::fmt::Arguments) {
    x86_64::debug_print_fmt(args);
}

pub unsafe fn force_enable_interrupts() {
    x86_64::force_enable_interrupts();
}

pub unsafe fn force_disable_interrupts() {
    x86_64::force_disable_interrupts();
}

pub fn is_interrupts_enabled() -> bool {
    x86_64::is_interrupts_enabled()
}
