//! Module to handle all the EFI interfaces
//! Spec: https://uefi.org/sites/default/files/resources/UEFI_Spec_2_9_2021_03_18.pdf

// TODO(patrik):
//   - Make custom error structure
//   - Convert EfiStatus to custom error

use core::sync::atomic::{ Ordering, AtomicPtr };

pub type Result<T> = core::result::Result<T, Error>;

static SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

#[derive(Debug)]
pub enum Error {
    SystemTableNotRegistered,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
#[repr(C)]
enum EfiAllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
    MaxAllocateType
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
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

#[derive(Copy, Clone, PartialEq, Debug)]
enum EfiWarning {
    /// The string contained one or more characters that the device could
    /// not render and were skipped.
    UnknownGlyph,

    /// The handle was closed, but the file was not deleted.
    DeleteFailure,

    /// The handle was closed, but the data to the file was not flushed
    /// properly.
    WriteFailure,

    /// The resulting buffer was too small, and the data was truncated to
    /// the buffer size.
    BufferTooSmall,

    /// The data has not been updated within the timeframe set by local
    /// policy for this type of data.
    StaleData,

    /// The resulting buffer contains UEFI-compliant file system.
    FileSystem,

    /// The operation will be processed across a system reset.
    ResetRequired,

    Unknown(u64),
}

impl From<u64> for EfiWarning {
    fn from(value: u64) -> Self {
        match value {
            1 => Self::UnknownGlyph,
            2 => Self::DeleteFailure,
            3 => Self::WriteFailure,
            4 => Self::BufferTooSmall,
            5 => Self::StaleData,
            6 => Self::FileSystem,
            7 => Self::ResetRequired,

            _ => Self::Unknown(value),
        }
    }
}

enum EfiError {
    Unknown(u64),
}

impl From<u64> for EfiError {
    fn from(value: u64) -> Self {
        match value {
            _ => EfiError::Unknown(value),
        }
    }
}

enum EfiStatus {
    Success,

    Warning(EfiWarning),
    Error(EfiError),

    Other(usize)
}

impl From<EfiStatusCode> for EfiStatus {
    fn from(value: EfiStatusCode) -> Self {
        // Sign extend the code so that if we are running 32-bit we can still
        // handle the same errors
        let code = value.0 as isize as i64 as u64;

        match value.0 {
            0 => Self::Success,

            0x0000000000000001..=0x1fffffffffffffff => {
                // EFI Warning
                let warning = EfiWarning::from(code);
                EfiStatus::Warning(warning)
            }

            0x8000000000000000..=0x9fffffffffffffff => {
                let code = code & !0x8000000000000000;
                let error = EfiError::from(code);
                EfiStatus::Error(error)
            }

            _ => Self::Other(value.0)
        }
    }
}

#[repr(transparent)]
pub struct EfiStatusCode(usize);

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
                                    string: *const u16) -> EfiStatusCode,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: unsafe extern fn(this: *mut EfiSimpleTextOutputProtocol)
                        -> EfiStatusCode,
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

pub fn clear_screen() -> Result<()> {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        ((*(*system_table).con_out).clear_screen)((*system_table).con_out);
    }

    Ok(())
}

pub fn output_string(buffer: &[u16]) -> Result<()> {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        ((*(*system_table).con_out).output_string)((*system_table).con_out,
                                                   buffer.as_ptr());
    }

    Ok(())
}
