#![no_std]
#![no_main]

use core::panic::PanicInfo;

type EfiHandle = usize;

#[repr(transparent)]
struct EfiStatus(usize);

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
struct EfiBootServices {
    header: EfiTableHeader,

    raise_tpl: usize,
    restore_tpl: usize,

    allocate_pages: usize,
    free_pages: usize,
    get_memory_map: usize,
    allocate_pool: usize,
    free_pool: usize,

    create_event: usize,
    set_timer: usize,
    wait_for_event: usize,
    signal_event: usize,
    close_event: usize,
    check_event: usize,

    install_protocol_interface: usize,
    reinstall_protocol_interface: usize,
    uninstall_protocol_interface: usize,
    handle_protocol: usize,
    reserved: usize,
    register_protocol_notify: usize,
    locate_handle: usize,
    locate_device_path: usize,
    install_configuration_table: usize,

    load_image: usize,
    start_image: usize,
    exit: usize,
    unload_image: usize,
    exit_boot_services: usize,

    get_next_monotonic_count: usize,
    stall: usize,
    set_watchdog_timer: usize,

    connect_controller: usize,
    disconnect_controller: usize,

    open_protocol: usize,
    close_protocol: usize,
    open_protocol_infomation: usize,

    procols_per_handle: usize,
    locate_handle_buffer: usize,
    locate_protocol: usize,
    install_multiple_protocol_interfaces: usize,
    uninstall_multiple_protocol_interface: usize,

    calculate_crc32: usize,

    copy_mem: usize,
    set_mem: usize,

    create_event_ex: usize,
}

#[repr(C)]
struct EfiSimpleTextOutputProtocol {
    reset: usize,
    output_string: unsafe fn(this: &EfiSimpleTextOutputProtocol, string: *const u16) -> EfiStatus,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: unsafe fn(this: &EfiSimpleTextOutputProtocol) -> EfiStatus,
    set_cursor_position: usize,
    enable_cursor: usize,
    mode: usize,
}

#[repr(C)]
struct EfiSystemTable {
    header: EfiTableHeader,

    firmware_vendor: usize,
    firmware_revision: u32,

    console_in_handle: EfiHandle,
    con_in: usize,

    console_out_handle: EfiHandle,
    con_out: &'static EfiSimpleTextOutputProtocol,

    standard_error_handle: EfiHandle,
    std_err: usize,

    runtime_services: usize,
    boot_services: &'static EfiBootServices,

    number_of_table_entries: usize,
    configuration_table: usize
}

#[no_mangle]
fn efi_main(_image_handle: usize, table: &EfiSystemTable) -> u64 {
    unsafe {
        (table.con_out.clear_screen)(&table.con_out);
        let data = [0x42u16,0x42u16,0x42u16,0x42u16,0x42u16,0x42u16,0xdu16,0xau16, 0x00u16];
        (table.con_out.output_string)(&table.con_out, data.as_ptr());
    }

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
