#![no_std]

use core::panic::PanicInfo;

#[no_mangle]
extern fn kernel_init() -> ! {
    let ptr = 0xb8000 as *mut u16;
    unsafe {
        *ptr.offset(0) = 0x1f41;
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
