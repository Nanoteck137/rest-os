//! Module to handle all the EFI interfaces
//!
//! Spec: <https://uefi.org/sites/default/files/resources/UEFI_Spec_2_9_2021_03_18.pdf>

#![allow(dead_code)]

// TODO(patrik):

use core::sync::atomic::{ Ordering, AtomicPtr };

/// The standered size of a page (4096 bytes)
const PAGE_SIZE: usize = 0x1000;

/// The GUID of the ACPI configuration table
const ACPI_20_TABLE_GUID: EfiGuid =
    EfiGuid::new(0x8868e871, 0xe4f1, 0x11d3,
                 [0xbc,0x22,0x00,0x80,0xc7,0x3c,0x88,0x81]);

pub type Result<T> = core::result::Result<T, Error>;

/// The Registered EFI system table pointer
static SYSTEM_TABLE: AtomicPtr<EfiSystemTable> =
    AtomicPtr::new(core::ptr::null_mut());

/// Errors defined by the EFI module
#[derive(Debug)]
pub enum Error {
    /// No system table has been registerd
    SystemTableNotRegistered,

    /// Failed to clear the screen
    ClearScreen(EfiStatus),

    /// Failed to output string to the screen
    OutputString(EfiStatus),

    /// Failed to allocate pages
    AllocatePages(EfiStatus),

    /// Failed to retrive the memory map
    MemoryMap(EfiStatus),

    /// Failed to exit boot services
    ExitBootServices(EfiStatus),

    /// The buffer was too small
    ByteBufferTooSmall,

    /// Unknown memory type
    UnknownMemoryType(u64),

    /// Unknown memory attribute
    UnknownMemoryAttribute(u64),

    /// Failed to find the ACPI table inside the EFI configuration tables
    UnableToFindACPITable,
}

/// EFI GUID 128-bit ID
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug)]
struct EfiGuid {
    part1: u32,
    part2: u16,
    part3: u16,
    part4: [u8; 8],
}

impl EfiGuid {
    const fn new(part1: u32, part2: u16, part3: u16, part4: [u8; 8]) -> Self {
        Self {
            part1,
            part2,
            part3,
            part4
        }
    }
}

/// Types of allocations we can do inside for example
/// [`EfiBootServices::allocate_pages`]
#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
#[repr(C)]
enum EfiAllocateType {
    /// Allocation requests of Type AllocateAnyPages allocate any available
    /// range of pages that satisfies the request. On input, the address
    /// pointed to by Memory is ignored.
    AnyPages,

    /// Allocation requests of Type AllocateMaxAddress allocate any available
    /// range of pages whose uppermost address is less than or equal to
    /// the address pointed to by Memory on input.
    MaxAddress,

    /// Allocation requests of Type AllocateAddress allocate pages at the
    /// address pointed to by Memory on input.
    Address,
}

/// Types of memory regions we can for example allocate from
#[derive(Copy, Clone, PartialEq, Debug)]
#[allow(dead_code)]
#[repr(C)]
pub enum EfiMemoryType {
    /// Not usable
    ReservedMemoryType,

    /// The code portions of a loaded UEFI application
    LoaderCode,

    /// The data portions of a loaded UEFI application and the default data
    /// allocation type used by a UEFI application to allocate pool memory
    LoaderData,

    /// The code portions of a loaded UEFI Boot Service Driver
    BootServicesCode,

    /// The data portions of a loaded UEFI Boot Serve Driver, and the default
    /// data allocation type used by a UEFI Boot Service Driver to allocate
    /// pool memory
    BootServicesData,

    /// The code portions of a loaded UEFI Runtime Driver
    RuntimeServicesCode,

    /// The data portions of a loaded UEFI Runtime Driver and the default
    /// data allocation type used by a UEFI Runtime Driver to allocate
    /// pool memory.
    RuntimeServicesData,

    /// Free (unallocated) memory
    ConventionalMemory,

    /// Memory in which errors have been detected
    UnusableMemory,

    /// Memory that holds the ACPI tables
    ACPIReclaimMemory,

    /// Address space reserved for use by the firmware
    ACPIMemoryNVS,

    /// Used by system firmware to request that a memory-mapped IO region
    /// be mapped by the OS to a virtual address so it can be accessed by
    /// EFI runtime services
    MemoryMappedIO,

    /// System memory-mapped IO region that is used to translate memory cycles
    /// to  IO cycles by the processor
    MemoryMappedIOPortSpace,

    /// Address space reserved by the firmware for code that is part of the
    /// processor
    PalCode,

    /// A memory region that operates as EfiConventionalMemory.
    /// However, it happens to also support byte-addressable non-volatility
    PersistentMemory,

    /// A memory region that represents unaccepted memory, that must be
    /// accepted by the boot target before it can be used. Unless otherwise
    /// noted, all other EFI memory types are accepted. For platforms that
    /// support unaccepted memory, all unaccepted valid memory will be
    /// reported as unaccepted in the memory map. Unreported physical address
    /// ranges must be treated as not-present memory.
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
    /// Attributes of the memory region that describe the bit mask of
    /// capabilities for that memory region, and not necessarily the current
    /// settings for that memory region.
    #[repr(transparent)]
    pub struct EfiMemoryAttribute: u64 {
        /// Memory cacheability attribute: The memory region supports being
        /// configured as not cacheable
        const UC  = 0x0000000000000001;

        /// Memory cacheability attribute: The memory region supports being
        /// configured as write combining.
        const WC  = 0x0000000000000002;

        /// Memory cacheability attribute: The memory region supports being
        /// configured as cacheable with a "write through" policy.
        /// Writes that hit in the cache will also be written to main memory.
        const WT  = 0x0000000000000004;

        /// Memory cacheability attribute: The memory region supports being
        /// configured as cacheable with a "write back" policy.
        /// Reads and writes that hit in the cache do not propagate to
        /// main memory. Dirty data is written back to main memory when a
        /// new cache line is allocated.
        const WB  = 0x0000000000000008;

        /// Memory cacheability attribute: The memory region supports being
        /// configured as not cacheable, exported, and supports the
        /// "fetch and add" semaphore mechanism.
        const UCE = 0x0000000000000010;

        /// Physical memory protection attribute: The memory region supports
        /// being configured as write-protected by system hardware.
        /// This is typically used as a cacheability attribute today.
        /// The memory region supports being configured as cacheable with
        /// a "write protected" policy. Reads come from cache lines when
        /// possible, and read misses cause cache fills. Writes are
        /// propagated to the system bus and cause corresponding cache lines
        /// on all processors on the bus to be invalidated.
        const WP  = 0x0000000000001000;

        /// Physical memory protection attribute: The memory region supports
        /// being configured as read-protected by system hardware.
        const RP = 0x0000000000002000;

        /// Physical memory protection attribute: The memory region supports
        /// being configured so it is protected by system hardware from
        /// executing code.
        const XP = 0x0000000000004000;

        /// Runtime memory attribute: The memory region refers to persistent
        /// memory
        const NV = 0x0000000000008000;

        /// The memory region provides higher reliability relative to other
        /// memory in the system. If all memory has the same reliability,
        /// then this bit is not used.
        const MORE_RELIABLE = 0x0000000000010000;

        /// Physical memory protection attribute: The memory region supports
        /// making this memory range read-only by system hardware.
        const RO            = 0x0000000000020000;

        /// Specific-purpose memory (SPM). The memory is earmarked for
        /// specific purposes such as for specific device drivers or
        /// applications. The SPM attribute serves as a hint to the OS to
        /// avoid allocating this memory for core OS data or code that can
        /// not be relocated. Prolonged use of this memory for purposes other
        /// than the intended purpose may result in suboptimal platform
        /// performance.
        const SP            = 0x0000000000040000;

        /// If this flag is set, the memory region is capable of being
        /// protected with the CPU's memory cryptographic capabilities.
        /// If this flag is clear, the memory region is not capable of being
        /// protected with the CPU's memory cryptographic capabilities or
        /// the CPU does not support CPU memory cryptographic capabilities
        const CPU_CRYPTO = 0x0000000000080000;

        /// Runtime memory attribute: The memory region needs to be given a
        /// virtual mapping by the operating system when
        /// SetVirtualAddressMap() is called
        const RUNTIME    = 0x8000000000000000;
    }
}

/// EFI Memory Descriptor is for describing diffrent memory regions
#[derive(Copy, Clone)]
#[repr(C)]
struct EfiMemoryDescriptor {
    /// Type of the memory region
    typ: u32,

    /// Physical address of the first byte in the memory region
    physical_start: u64,

    /// Virtual address of the first byte in the memory region
    virtual_start: u64,

    /// Number of 4 KiB pages in the memory region
    num_pages: u64,

    /// Attributes of the memory region
    attribute: u64,
}

/// EFI standard warnings
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

/// EFI standard errors
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

/// Status for the EFI interfaces, converted from [`EfiStatusCode`]
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

/// EFI status code returned by most EFI inteface functions
#[repr(transparent)]
pub struct EfiStatusCode(usize);

// TODO(patrik): Should EfiHandle derive from Copy and Clone?
/// EFI handle
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct EfiHandle(usize);

/// Holds a pointer to the [`EfiSystemTable`], then later can be registered
/// by the [`EfiSystemTablePtr::register`] function to register this system
/// table to the global system table handle
#[repr(transparent)]
pub struct EfiSystemTablePtr(*mut EfiSystemTable);

impl EfiSystemTablePtr {
    /// Register a EfiSystemTablePtr to the global system table handle
    pub unsafe fn register(self) {
        SYSTEM_TABLE.store(self.0, Ordering::SeqCst);
    }
}

/// EFI Table header
#[repr(C)]
struct EfiTableHeader {
    /// A 64-bit signature that identifies the type of table that follows
    signature: u64,

    /// The revision of the EFI Specification to which this table conforms
    revision: u32,

    /// The size, in bytes, of the entire table including the
    /// [`EfiTableHeader`].
    header_size: u32,

    /// The 32-bit CRC for the entire table
    crc32: u32,

    /// Reserved field that must be set to 0
    reserved: u32,
}

/// EFI BootServices table
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

/// EFI Simple Text Output Protocol
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

/// Contains a set of GUID/pointer pairs comprised of the ConfigurationTable
/// field in the EFI System Table
#[repr(C)]
struct EfiConfigurationTable {
    /// The 128-bit GUID value that uniquely identifies the system
    /// configuration table.
    vendor_guid: EfiGuid,

    /// A pointer to the table associated with VendorGuid
    vendor_table: *mut u8,
}

/// EFI System table
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
    configuration_table: *const EfiConfigurationTable,
}

/// Clears the screen of the EFI stdout
///
/// # Returns
///
/// * `Ok(())` - The clearing of the screen was successful
/// * `Err` - The clearing of the screen failed
pub fn clear_screen() -> Result<()> {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        let status: EfiStatus =
            ((*(*system_table).con_out).clear_screen)(
                (*system_table).con_out).into();

        if status != EfiStatus::Success {
            return Err(Error::ClearScreen(status));
        }
    }

    Ok(())
}

/// Outputs a UTF-16 string to the EFI stdout
///
/// # Arguments
///
/// * `buffer` - The UTF-16 string buffer to be printed
///
/// # Returns
///
/// * `Ok(())` - The print was successful
/// * `Err` - The print failed
pub fn output_string(buffer: &[u16]) -> Result<()> {
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        let status: EfiStatus =
            ((*(*system_table).con_out).output_string)(
                (*system_table).con_out,
                buffer.as_ptr()).into();

        if status != EfiStatus::Success {
            return Err(Error::OutputString(status));
        }
    }

    Ok(())
}

/// Allocates a contiguous number of pages from the `LoaderData`
///
/// # Arguments
///
/// * `num_pages` - The number of pages to allocate
///
/// # Returns
/// * `Ok(addr)` - If the allocation was succeccful then we return the address
///    of the first page.
/// * `Err` - If the allocation failed then we return a error
pub fn allocate_pages(num_pages: usize) -> Result<usize> {
    // Get access to the system table
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);

    // Check if it's registered
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    // We use 'AllocateAnyPages' because we don't care where the pages
    // are located
    let typ = EfiAllocateType::AnyPages;

    // We allocate from the LoaderData because the UEFI spec
    // recommends to use that when we are executing as
    // UEFI application/loader
    let memory_type = EfiMemoryType::LoaderCode;

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

/// Memory Descriptor describe a region of memory parsed from
/// EfiMemoryDescriptor
#[derive(Copy, Clone, Debug)]
pub struct MemoryDescriptor {
    /// The type of memory region
    typ: EfiMemoryType,

    /// The start address of the memory region (physical address)
    start: u64,

    /// The length of the memory region (bytes)
    length: u64,

    /// The attributes of the memory region
    attribute: EfiMemoryAttribute
}

impl MemoryDescriptor {
    /// Parses a byte buffer and returns the MemoryDescriptor
    ///
    /// # Arguments
    ///
    /// * `bytes` - A reference to a slice of bytes to parse
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

    /// Returns the type of the region
    pub fn typ(&self) -> EfiMemoryType {
        self.typ
    }

    /// Returns the start address of the region (physcial address)
    pub fn start(&self) -> u64 {
        self.start
    }

    /// Returns the end address of the byte +1 past the region
    /// (physcial address)
    pub fn end(&self) -> u64 {
        self.start + self.length
    }

    /// Returns the length of the regoin (bytes)
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Returns the attributes of the region
    pub fn attribute(&self) -> EfiMemoryAttribute {
        self.attribute
    }
}

/// Retrive the current memory map from EFI
///
/// # Arguments
///
/// * `buffer` - The buffer to fill the memory map descriptor data
///
/// # Returns
///
/// * `Ok((memory_map_size, map_key, descriptor_size))` - If the retrival of
///    the memory map was successful then we return a tuple with the
///    `memory_map_size`, `map_key` and the `descriptor_size`
/// * `Err` - If we failed to retrive the memory we return the error
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

/// Find and return the address of the ACPI RSDP
///
/// # Returns
///
/// * `Ok(())` - If we succeeded to find a ACPI RSDP address
/// * `Err` - If we failed to find a ACPI RSDP address
///   - [`Error::SystemTableNotRegistered`] - If their is not a system table currently registered
///   - [`Error::UnableToFindACPITable`] - If we were unable to find the acpi rsdp
pub fn find_acpi_table() -> Result<usize> {
    // Get access to the system table
    let system_table = SYSTEM_TABLE.load(Ordering::SeqCst);

    // Check if it's registered
    if system_table.is_null() { return Err(Error::SystemTableNotRegistered) }

    unsafe {
        // The number of configuration tables from system
        let num_tables = (*system_table).number_of_table_entries;

        let mut rsdp = None;

        // Loop through all the configuration tables to find the ACPI table
        for i in 0..num_tables {
            // Get the pointer to the table
            let table_ptr = (*system_table).configuration_table.add(i);

            // The guid of the current table
            let table_guid = (*table_ptr).vendor_guid;
            // If the guid matches the ACPI GUID then check if that is a
            // valid RSDP signature
            if table_guid == ACPI_20_TABLE_GUID {
                // Get the header signature
                let header_sig = core::ptr::read((*table_ptr).vendor_table as *const [u8; 8]);
                // Check the signature
                if &header_sig == b"RSD PTR " {
                    // We found the right table
                    rsdp = Some((*table_ptr).vendor_table);
                    break;
                }
            }
        }

        // Check if we found the ACPI RSDP
        if let Some(rsdp) = rsdp {
            // Return the address
            Ok(rsdp as usize)
        } else {
            // Return a error if we did not find the RSDP
            Err(Error::UnableToFindACPITable)
        }
    }
}

/// Exit the boot services
///
/// # Arguments
///
/// * `image_handle` - The current image handle
/// * `map_key` - The newest map key retrived from the memory map
///
/// # Returns
///
/// * `Ok(())` - If we succeeded to exit the boot services
/// * `Err` - If we failed to exit the boot services
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
