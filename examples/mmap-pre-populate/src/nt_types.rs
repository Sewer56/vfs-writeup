// NT API type definitions and resolution utilities

use std::ffi::c_void;
use std::mem;
use windows::core::PCSTR;
use windows::Win32::Foundation::{HANDLE, HMODULE, NTSTATUS};
use windows::Win32::System::LibraryLoader::GetProcAddress;

// Type definitions for NT APIs
pub(crate) type NtCreateSectionFn = unsafe extern "system" fn(
    section_handle: *mut HANDLE,
    desired_access: u32,
    object_attributes: *mut c_void,
    maximum_size: *mut i64,
    section_page_protection: u32,
    allocation_attributes: u32,
    file_handle: HANDLE,
) -> NTSTATUS;

pub(crate) type NtMapViewOfSectionFn = unsafe extern "system" fn(
    section_handle: HANDLE,
    process_handle: HANDLE,
    base_address: *mut *mut c_void,
    zero_bits: usize,
    commit_size: usize,
    section_offset: *mut i64,
    view_size: *mut usize,
    inherit_disposition: u32,
    allocation_type: u32,
    win32_protect: u32,
) -> NTSTATUS;

pub(crate) type NtUnmapViewOfSectionFn =
    unsafe extern "system" fn(process_handle: HANDLE, base_address: *mut c_void) -> NTSTATUS;

pub(crate) type NtUnmapViewOfSectionExFn = unsafe extern "system" fn(
    process_handle: HANDLE,
    base_address: *mut c_void,
    flags: u32,
) -> NTSTATUS;

pub(crate) type NtCloseFn = unsafe extern "system" fn(handle: HANDLE) -> NTSTATUS;

/// Resolves and transmutes a function from a loaded module
pub(crate) unsafe fn resolve_nt_function<T>(module: HMODULE, name: &str) -> T {
    let name_cstr =
        std::ffi::CString::new(name).unwrap_or_else(|_| panic!("Invalid function name: {}", name));
    let addr = GetProcAddress(module, PCSTR(name_cstr.as_ptr() as *const u8))
        .unwrap_or_else(|| panic!("Failed to resolve {}", name));
    mem::transmute_copy(&addr)
}
