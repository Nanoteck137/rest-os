//! Module to handle all the EFI interfaces

// TODO(patrik):
//   - Make custom error structure
//   - Convert EfiStatus to custom error

use core::sync::atomic::{ Ordering, AtomicPtr };

static SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

#[repr(transparent)]
pub struct EfiStatus(usize);

#[repr(transparent)]
pub struct EfiHandle(usize);

#[repr(transparent)]
pub struct EfiSystemTablePtr(*mut EfiSystemTable);

impl EfiSystemTablePtr {
    pub unsafe fn register(self) {
        SYSTEM_TABLE.store(self.0, Ordering::SeqCst);
    }
}

#[repr(C)]
struct EfiTableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(C)]
enum EfiAllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType
};

#[derive(Copy, Clone, PartialEq, Debug)]
#[repr(C)]
enum EfiMemoryType {
    ReservedMemoryType,
    LoaderCode,
    LoaderData,
    BootServicesCode,
    BootServicesData,
    RuntimeServicesCode,
    RuntimeServicesData,
    ConventionalMemory,
    UnusableMemory,
    ACPIReclaimMemory,

    ACPIMemoryNVS,
    MemoryMappedIO,
    MemoryMappedIOPortSpace,

    PalCode,
    PersistentMemory,
    UnacceptedMemoryType
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
    output_string: unsafe extern fn(this: *mut EfiSimpleTextOutputProtocol,
                                    string: *const u16) -> EfiStatus,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: unsafe extern fn(this: *mut EfiSimpleTextOutputProtocol)
                        -> EfiStatus,
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
    con_out: *mut EfiSimpleTextOutputProtocol,

    standard_error_handle: EfiHandle,
    std_err: usize,

    runtime_services: usize,
    boot_services: *mut EfiBootServices,

    number_of_table_entries: usize,
    configuration_table: usize
}

pub fn clear_screen() {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    assert!(!system_table.is_null(), "No System table has been registerd");

    unsafe {
        ((*(*system_table).con_out).clear_screen)((*system_table).con_out);
    }
}

pub fn output_string(buffer: &[u16]) {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    assert!(!system_table.is_null(), "No System table has been registerd");

    unsafe {
        ((*(*system_table).con_out).output_string)((*system_table).con_out,
                                                   buffer.as_ptr());
    }
}
