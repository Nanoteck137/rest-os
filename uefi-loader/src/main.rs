#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
fn efi_main(_image_handle: usize, _table: usize) -> u64 {
    0
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
