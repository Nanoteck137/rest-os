//! Module to handle all the EFI interfaces
//!
//! Spec: <https://uefi.org/sites/default/files/resources/UEFI_Spec_2_9_2021_03_18.pdf>

// TODO(patrik):

use core::sync::atomic::{ Ordering, AtomicPtr };

const PAGE_SIZE: usize = 0x1000;

pub type Result<T> = core::result::Result<T, Error>;

static SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

#[derive(Debug)]
pub enum Error {
    SystemTableNotRegistered,
    AllocatePages(EfiStatus),
    MemoryMap(EfiStatus),
    ExitBootServices(EfiStatus),

    ByteBufferTooSmall,
    UnknownMemoryType(u64),
    UnknownMemoryAttribute(u64),
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
#[repr(C)]
enum EfiAllocateType {
    AllocateAnyPages,
    AllocateMaxAddress,
    AllocateAddress,
}

#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
#[repr(C)]
pub enum EfiMemoryType {
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

impl TryFrom<u64> for EfiMemoryType {
    type Error = Error;
    fn try_from(value: u64) -> Result<Self> {
        match value {
            0 => Ok(Self::ReservedMemoryType),
            1 => Ok(Self::LoaderCode),
            2 => Ok(Self::LoaderData),
            3 => Ok(Self::BootServicesCode),
            4 => Ok(Self::BootServicesData),
            5 => Ok(Self::RuntimeServicesCode),
            6 => Ok(Self::RuntimeServicesData),
            7 => Ok(Self::ConventionalMemory),
            8 => Ok(Self::UnusableMemory),
            9 => Ok(Self::ACPIReclaimMemory),

            10 => Ok(Self::ACPIMemoryNVS),
            11 => Ok(Self::MemoryMappedIO),
            12 => Ok(Self::MemoryMappedIOPortSpace),

            13 => Ok(Self::PalCode),
            14 => Ok(Self::PersistentMemory),
            15 => Ok(Self::UnacceptedMemoryType),

            _ => Err(Error::UnknownMemoryType(value)),
        }
    }
}

bitflags! {
    #[repr(transparent)]
    pub struct EfiMemoryAttribute: u64 {
        const UC  = 0x0000000000000001;
        const WC  = 0x0000000000000002;
        const WT  = 0x0000000000000004;
        const WB  = 0x0000000000000008;
        const UCE = 0x0000000000000010;
        const WP  = 0x0000000000001000;

        const RP = 0x0000000000002000;
        const XP = 0x0000000000004000;
        const NV = 0x0000000000008000;

        const MORE_RELIABLE = 0x0000000000010000;
        const RO            = 0x0000000000020000;
        const SP            = 0x0000000000040000;

        const CPU_CRYPTO = 0x0000000000080000;
        const RUNTIME    = 0x8000000000000000;
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
struct EfiMemoryDescriptor {
    typ: u32,
    physical_start: u64,
    virtual_start: u64,
    num_pages: u64,
    attribute: u64,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EfiWarning {
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

    /// Unknown EFI warning
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

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EfiError {
    /// The image failed to load.
    LoadError,

    /// A parameter was incorrect.
    InvalidParameter,

    /// The operation is not supported.
    Unsupported,

    /// The buffer was not the proper size for the request.
    BadBufferSize,

    /// The buffer is not large enough to hold the requested data.
    /// The required buffer size is returned in the appropriate parameter
    /// when this error occurs.
    BufferTooSmall,

    /// There is no data pending upon return.
    NotReady,

    /// The physical device reported an error while attempting the operation.
    DeviceError,

    /// The device cannot be written to.
    WriteProtected,

    /// A resource has run out.
    OutOfResources,

    /// An inconstancy was detected on the file system causing the operating
    /// to fail.
    VolumeCorrupted,

    /// There is no more space on the file system.
    VolumeFull,

    /// The device does not contain any medium to perform the operation.
    NoMedia,

    /// The medium in the device has changed since the last access.
    MediaChanged,

    /// The item was not found.
    NotFound,

    /// Access was denied.
    AccessDenied,

    /// The server was not found or did not respond to the request.
    NoResponse,

    /// A mapping to a device does not exist.
    NoMapping,

    /// The timeout time expired.
    Timeout,

    /// The protocol has not been started.
    NotStarted,

    /// The protocol has already been started.
    AlreadyStarted,

    /// The operation was aborted.
    Aborted,

    /// An ICMP error occurred during the network operation.
    IcmpError,

    /// A TFTP error occurred during the network operation.
    TftpError,

    /// A protocol error occurred during the network operation.
    ProtocolError,

    /// The function encountered an internal version that was incompatible
    /// with a version requested by the caller.
    IncompatibleVersion,

    /// The function was not performed due to a security violation.
    SecurityViolation,

    /// A CRC error was detected.
    CrcError,

    /// Beginning or end of media was reached
    EndOfMedia,

    /// The end of the file was reached.
    EndOfFile,

    /// The language specified was invalid.
    InvalidLanguage,

    /// The security status of the data is unknown or compromised and the
    /// data must be updated or replaced to restore a valid security status.
    CompromisedData,

    /// There is an address conflict address allocation
    IpAddressConflict,

    /// A HTTP error occurred during the network operation.
    HttpError,

    /// Unknown EFI error
    Unknown(u64),
}

impl From<u64> for EfiError {
    fn from(value: u64) -> Self {
        match value {
            1 => Self::LoadError,
            2 => Self::InvalidParameter,
            3 => Self::Unsupported,
            4 => Self::BadBufferSize,
            5 => Self::BufferTooSmall,
            6 => Self::NotReady,
            7 => Self::DeviceError,
            8 => Self::WriteProtected,
            9 => Self::OutOfResources,
            10 => Self::VolumeCorrupted,
            11 => Self::VolumeFull,
            12 => Self::NoMedia,
            13 => Self::MediaChanged,
            14 => Self::NotFound,
            15 => Self::AccessDenied,
            16 => Self::NoResponse,
            17 => Self::NoMapping,
            18 => Self::Timeout,
            19 => Self::NotStarted,
            20 => Self::AlreadyStarted,
            21 => Self::Aborted,
            22 => Self::IcmpError,
            23 => Self::TftpError,
            24 => Self::ProtocolError,
            25 => Self::IncompatibleVersion,
            26 => Self::SecurityViolation,
            27 => Self::CrcError,
            28 => Self::EndOfMedia,
            31 => Self::EndOfFile,
            32 => Self::InvalidLanguage,
            33 => Self::CompromisedData,
            34 => Self::IpAddressConflict,
            35 => Self::HttpError,

            _ => EfiError::Unknown(value),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum EfiStatus {
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

// TODO(patrik): Should EfiHandle derive from Copy and Clone?
#[derive(Copy, Clone)]
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

    allocate_pages: unsafe extern fn(EfiAllocateType,
                                     EfiMemoryType,
                                     usize,
                                     &mut usize) -> EfiStatusCode,
    free_pages: usize,
    get_memory_map: unsafe extern fn(memory_map_size: *mut usize,
                                     memory_map: *mut u8,
                                     map_key: *mut usize,
                                     descriptor_size: *mut usize,
                                     descriptor_version: *mut u32)
                                        -> EfiStatusCode,
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
    exit_boot_services: unsafe extern fn(image_handle: EfiHandle,
                                         map_key: usize) -> EfiStatusCode,

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

pub fn allocate_pages(num_pages: usize) -> Result<usize> {
    // Get access to the system table
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);

    // Check if it's registered
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    // We use 'AllocateAnyPages' because we don't care where the pages
    // are located
    let typ = EfiAllocateType::AllocateAnyPages;

    // We allocate from the LoaderData because the UEFI spec
    // recommends to use that when we are executing as
    // UEFI application/loader
    let memory_type = EfiMemoryType::LoaderData;

    // The address we got from `allocate_pages`
    let mut addr = 0usize;

    unsafe {
        // Allocate some pages
        let status: EfiStatus =
            ((*(*system_table).boot_services).allocate_pages)(
                typ, memory_type,
                num_pages, &mut addr).into();

        // Check if the call to `allocate_pages` where successful
        if status != EfiStatus::Success {
            return Err(Error::AllocatePages(status));
        }
    }

    // Return the address we got from `allocate_pages`
    Ok(addr)
}

#[derive(Copy, Clone, Debug)]
pub struct MemoryDescriptor {
    typ: EfiMemoryType,
    start: u64,
    length: u64,
    attribute: EfiMemoryAttribute
}

impl MemoryDescriptor {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < core::mem::size_of::<EfiMemoryAttribute>() {
            return Err(Error::ByteBufferTooSmall);
        }

        let descriptor = unsafe {
            core::ptr::read(bytes.as_ptr() as *const EfiMemoryDescriptor)
        };

        let typ = EfiMemoryType::try_from(descriptor.typ as u64)?;
        let attribute = EfiMemoryAttribute::from_bits(descriptor.attribute)
            .ok_or(Error::UnknownMemoryAttribute(descriptor.attribute))?;

        let start = descriptor.physical_start;
        let length = descriptor.num_pages * PAGE_SIZE as u64;

        Ok(Self {
            typ,
            start,
            length,
            attribute
        })
    }

    pub fn typ(&self) -> EfiMemoryType {
        self.typ
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.start + self.length
    }

    pub fn length(&self) -> u64 {
        self.length
    }

    pub fn attribute(&self) -> EfiMemoryAttribute {
        self.attribute
    }
}

pub fn memory_map(buffer: &mut [u8]) -> Result<(usize, usize, usize)> {
    // Get access to the system table
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);

    // Check if it's registered
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    let mut memory_map_size: usize = buffer.len();
    let mut map_key: usize = 0;
    let mut descriptor_size: usize = 0;
    let mut descriptor_version: u32 = 0;

    unsafe {
        let status: EfiStatus =
            ((*(*system_table).boot_services).get_memory_map)(
                core::ptr::addr_of_mut!(memory_map_size),
                buffer.as_mut_ptr(),
                core::ptr::addr_of_mut!(map_key),
                core::ptr::addr_of_mut!(descriptor_size),
                core::ptr::addr_of_mut!(descriptor_version)).into();
        if status != EfiStatus::Success {
            return Err(Error::MemoryMap(status));
        }
    }

    Ok((memory_map_size, map_key, descriptor_size))
}

pub fn exit_boot_services(image_handle: EfiHandle, map_key: usize)
    -> Result<()>
{
    // Get access to the system table
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);

    // Check if it's registered
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        let status: EfiStatus =
            ((*(*system_table).boot_services).exit_boot_services)(
                image_handle,
                map_key).into();
        if status != EfiStatus::Success {
            return Err(Error::ExitBootServices(status));
        }
    }

    Ok(())
}
