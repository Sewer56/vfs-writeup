// Hook implementations, state management, and initialisation

use crate::content;
use crate::nt_types::*;
use retour::RawDetour;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Foundation::{HANDLE, NTSTATUS};
use windows::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH,
    EXCEPTION_POINTERS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_NOACCESS, PAGE_READWRITE,
};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

// Virtual file marker handle (synthetic pointer value representing virtual file)
const VIRTUAL_FILE_MARKER: usize = 0xDEADBEEF;

// System page size and file size (initialised from GetSystemInfo)
static mut PAGE_SIZE: usize = 0;
static mut FILE_SIZE: usize = 0; // 6 pages

// Initialise page size from system
unsafe fn init_page_size() {
    let mut system_info = SYSTEM_INFO::default();
    GetSystemInfo(&mut system_info);
    PAGE_SIZE = system_info.dwPageSize as usize;
    FILE_SIZE = PAGE_SIZE * 6; // 6 pages
}

// Original function pointers (pointers to the original unhooked functions)
static mut ORIGINAL_NT_CREATE_SECTION: Option<NtCreateSectionFn> = None;
static mut ORIGINAL_NT_MAP_VIEW: Option<NtMapViewOfSectionFn> = None;
static mut ORIGINAL_NT_UNMAP_VIEW: Option<NtUnmapViewOfSectionFn> = None;
static mut ORIGINAL_NT_UNMAP_VIEW_EX: Option<NtUnmapViewOfSectionExFn> = None;
static mut ORIGINAL_NT_CLOSE: Option<NtCloseFn> = None;

// Static detours (kept alive to maintain hooks)
static mut NT_CREATE_SECTION_DETOUR: Option<RawDetour> = None;
static mut NT_MAP_VIEW_DETOUR: Option<RawDetour> = None;
static mut NT_UNMAP_VIEW_DETOUR: Option<RawDetour> = None;
static mut NT_UNMAP_VIEW_EX_DETOUR: Option<RawDetour> = None;
static mut NT_CLOSE_DETOUR: Option<RawDetour> = None;

// Exception handler handle wrapper (implements Send for thread safety)
#[allow(dead_code)]
struct HandlerHandle(*mut c_void);
unsafe impl Send for HandlerHandle {}

// Exception handler handle (stored to allow cleanup)
static EXCEPTION_HANDLER_HANDLE: Mutex<Option<HandlerHandle>> = Mutex::new(None);

// Tracking state for section handles, memory mappings, and exception ranges
struct MappingState {
    // Maps section handle -> section handle (for consistency with pre-populate)
    sections: HashMap<usize, usize>,
    // Maps base address -> (size, section handle)
    mappings: HashMap<usize, (usize, usize)>,
    // Maps memory base -> (size, section handle, offset) for exception handler lookup
    // Offset field added to support mapping at non-zero offsets
    exception_ranges: HashMap<usize, (usize, usize, i64)>,
}

// Use lazy initialisation for the global state
static MAPPING_STATE: Mutex<Option<MappingState>> = Mutex::new(None);

fn get_mapping_state() -> std::sync::MutexGuard<'static, Option<MappingState>> {
    let mut guard = MAPPING_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(MappingState {
            sections: HashMap::new(),
            mappings: HashMap::new(),
            exception_ranges: HashMap::new(),
        });
    }
    guard
}

// VectoredExceptionHandler: Intercepts page faults and commits pages on-demand
unsafe extern "system" fn exception_handler(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    if exception_info.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let exception_record = (*exception_info).ExceptionRecord;
    if exception_record.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    // Check if this is an access violation (0xC0000005)
    const EXCEPTION_ACCESS_VIOLATION: i32 = 0xC0000005u32 as i32;
    if (*exception_record).ExceptionCode.0 != EXCEPTION_ACCESS_VIOLATION {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    // Extract fault address from ExceptionInformation[1] (address being accessed)
    let fault_addr = (*exception_record).ExceptionInformation[1];

    // Acquire mapping state lock
    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if fault address is in any tracked exception range
    let mut found_range: Option<(usize, usize, usize, i64)> = None; // (base, size, section, offset)
    for (&base_addr, &(size, section, offset)) in &state.exception_ranges {
        if fault_addr >= base_addr && fault_addr < base_addr + size {
            found_range = Some((base_addr, size, section, offset));
            break;
        }
    }

    let (base_addr, _size, _section, offset) = match found_range {
        Some(range) => range,
        None => return EXCEPTION_CONTINUE_SEARCH, // Not our virtual file
    };

    // Calculate which page was accessed, accounting for offset
    // The page number for synthesis represents the file-relative page being accessed.
    // The page start address is relative to the view's base address (reserved memory).
    // Example: If mapping starts at offset PAGE_SIZE (page 1 of file),
    // then accessing byte 0 of the view (base_addr + 0) should retrieve file page 1.
    let view_page_num = (fault_addr - base_addr) / PAGE_SIZE;
    let file_page_num = view_page_num + ((offset as usize) / PAGE_SIZE);
    let page_start_addr = base_addr + (view_page_num * PAGE_SIZE);

    // Commit the page with VirtualAlloc
    let result = VirtualAlloc(
        Some(page_start_addr as *const c_void),
        PAGE_SIZE,
        MEM_COMMIT,
        PAGE_READWRITE,
    );

    if result.is_null() {
        // Commit failed - return CONTINUE_SEARCH
        return EXCEPTION_CONTINUE_SEARCH;
    }

    // Populate the page with synthesised content
    // In a real VFS hook, we would read the corresponding page from the virtual file to fill in.
    // We would probably also want to try preloading adjacent pages for performance.
    let buffer = slice::from_raw_parts_mut(page_start_addr as *mut u8, PAGE_SIZE);
    content::synthesise_page(buffer, file_page_num);

    println!(
        "      → Page fault handler: Committed and populated file page {} at address 0x{:X}",
        file_page_num, page_start_addr
    );

    // Return EXCEPTION_CONTINUE_EXECUTION to retry the faulting instruction
    EXCEPTION_CONTINUE_EXECUTION
}

// NtCreateSection hook implementation
unsafe extern "system" fn nt_create_section_detour(
    section_handle: *mut HANDLE,
    desired_access: u32,
    object_attributes: *mut c_void,
    maximum_size: *mut i64,
    section_page_protection: u32,
    allocation_attributes: u32,
    file_handle: HANDLE,
) -> NTSTATUS {
    // PROOF OF CONCEPT: This hardcoded marker check is a simplified demonstration.
    // In a real implementation, this would be a call to our VFS to check if this is a virtual file.
    // Production code would query the Layer 1 VFS registry: `if vfs_layer1::is_virtual_file(file_handle) { ... }`
    if file_handle.0 as usize == VIRTUAL_FILE_MARKER {
        println!("      → NtCreateSection hook: Virtual file detected");

        // CRITICAL DIFFERENCE from pre-populate: NO memory allocation at this stage
        println!("      → Creating anonymous section (no memory allocation yet)");

        // Create a synthetic section handle (use timestamp-based value to ensure uniqueness)
        let section = HANDLE(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos() as usize) as *mut c_void,
        );
        *section_handle = section;

        // Track section in mapping state
        let mut state_guard = get_mapping_state();
        let state = state_guard.as_mut().unwrap();
        let section_addr = section.0 as usize;
        state.sections.insert(section_addr, section_addr);

        println!("      → Section handle created and tracked");

        return NTSTATUS(0); // STATUS_SUCCESS
    }

    // Not our virtual file - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_CREATE_SECTION.unwrap();
    original_fn(
        section_handle,
        desired_access,
        object_attributes,
        maximum_size,
        section_page_protection,
        allocation_attributes,
        file_handle,
    )
}

// NtClose hook implementation
// This hook is critical for proper resource cleanup. Section objects have independent lifetimes
// from their mapped views. A section can have multiple views created and destroyed via
// NtMapViewOfSection/NtUnmapViewOfSection, but the section itself persists until the section
// handle is closed via NtClose. Memory allocated during NtMapViewOfSection must only be freed
// when the section handle is closed, not when individual views are unmapped.
//
// IMPORTANT: This implementation handles both normal and abnormal lifecycle patterns:
// - Normal: Views are unmapped (NtUnmapViewOfSection) before closing the section handle
// - Abnormal: Section handle is closed without unmapping views first
// In the abnormal case, we forcibly clean up all mappings, exception ranges, and committed
// pages to prevent stale references. However, if the application attempts to access the
// freed memory afterward, it will cause undefined behaviour. Applications should always
// follow proper Windows semantics and unmap views before closing section handles.
unsafe extern "system" fn nt_close_detour(handle: HANDLE) -> NTSTATUS {
    let handle_value = handle.0 as usize;

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if this handle is a tracked section handle
    if state.sections.remove(&handle_value).is_some() {
        println!("      → NtClose hook: Virtual section handle detected");

        // Find all mappings that use this section handle (handles both normal and abnormal lifecycle)
        let mut mappings_to_remove = Vec::new();
        for (&base_addr, &(_size, section_handle)) in &state.mappings {
            if section_handle == handle_value {
                mappings_to_remove.push(base_addr);
            }
        }

        // Warn if mappings still exist (indicates abnormal lifecycle)
        if !mappings_to_remove.is_empty() {
            println!(
                "      → Found {} mapping(s) to clean up",
                mappings_to_remove.len()
            );
        }

        // Clean up all mappings associated with this section
        for base_addr in &mappings_to_remove {
            // Remove tracking to prevent stale references
            state.exception_ranges.remove(base_addr);
            state.mappings.remove(base_addr);

            println!(
                "      → Deregistered exception range for base 0x{:X}",
                base_addr
            );
        }

        drop(state_guard);

        // Free the reserved memory for all mappings
        for base_addr in mappings_to_remove {
            VirtualFree(base_addr as *mut c_void, 0, MEM_RELEASE).expect("VirtualFree failed");
            println!("      → Freed memory at 0x{:X}", base_addr);
        }

        println!("      → Section cleaned up, memory freed");

        return NTSTATUS(0); // STATUS_SUCCESS
    }

    drop(state_guard);

    // Not our virtual section - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_CLOSE.unwrap();
    original_fn(handle)
}

// NtMapViewOfSection hook implementation
unsafe extern "system" fn nt_map_view_of_section_detour(
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
) -> NTSTATUS {
    let section_addr = section_handle.0 as usize;

    let state_guard = get_mapping_state();
    let state = state_guard.as_ref().unwrap();

    // Check if this is our virtual section
    if state.sections.contains_key(&section_addr) {
        println!("      → NtMapViewOfSection hook: Virtual section detected");

        // PROOF OF CONCEPT: Basic offset support demonstrated.
        // Read the section_offset parameter. In production, additional validation would be needed
        // (alignment checks, bounds checking, offset + view_size <= file_size, etc.)
        let offset = if !section_offset.is_null() {
            unsafe { *section_offset }
        } else {
            0
        };

        drop(state_guard);

        // CRITICAL DIFFERENCE from pre-populate: Allocate MEM_RESERVE only (not MEM_COMMIT)
        // Adjust the reserved size based on offset
        let adjusted_size = FILE_SIZE.saturating_sub(offset as usize);
        let memory_base = VirtualAlloc(None, adjusted_size, MEM_RESERVE, PAGE_NOACCESS);

        if memory_base.is_null() {
            return NTSTATUS(-1073741823); // STATUS_UNSUCCESSFUL
        }

        let memory_addr = memory_base as usize;

        println!(
            "      → Allocated MEM_RESERVE ({} bytes) at 0x{:X} (offset: {} bytes)",
            adjusted_size, memory_addr, offset
        );

        // Set output parameters
        *base_address = memory_base;
        if !view_size.is_null() {
            *view_size = adjusted_size;
        }

        // Register address range with exception handler (including offset for page calculation)
        let mut state_guard = get_mapping_state();
        let state = state_guard.as_mut().unwrap();
        state
            .exception_ranges
            .insert(memory_addr, (adjusted_size, section_addr, offset));
        state
            .mappings
            .insert(memory_addr, (adjusted_size, section_addr));

        println!(
            "      → Address range registered with exception handler: [0x{:X}, 0x{:X})",
            memory_addr,
            memory_addr + adjusted_size
        );
        println!("      → Mapping tracked (lazy commitment enabled)");

        return NTSTATUS(0); // STATUS_SUCCESS
    }
    drop(state_guard);

    // Not our virtual section - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_MAP_VIEW.unwrap();
    original_fn(
        section_handle,
        process_handle,
        base_address,
        zero_bits,
        commit_size,
        section_offset,
        view_size,
        inherit_disposition,
        allocation_type,
        win32_protect,
    )
}

// NtUnmapViewOfSection hook implementation
// This hook removes the view mapping tracking and deregisters the exception range, but does NOT
// free memory or remove the section from tracking. The section object persists independently of
// mapped views - multiple views can be created and destroyed from the same section. Memory
// deallocation happens in NtClose when the section handle itself is closed.
unsafe extern "system" fn nt_unmap_view_of_section_detour(
    process_handle: HANDLE,
    base_address: *mut c_void,
) -> NTSTATUS {
    let addr = base_address as usize;

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if this is our virtual mapping
    if state.mappings.contains_key(&addr) {
        println!("      → NtUnmapViewOfSection hook: Virtual mapping detected");

        // Deregister exception range (to prevent handler from processing faults after unmapping)
        state.exception_ranges.remove(&addr);

        // Remove mapping tracking (the view is unmapped, but memory persists until NtClose)
        state.mappings.remove(&addr);

        println!("      → Address range deregistered from exception handler");
        println!("      → View unmapped (memory will be freed when section handle is closed)");

        drop(state_guard);

        return NTSTATUS(0); // STATUS_SUCCESS
    }
    drop(state_guard);

    // Not our virtual mapping - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_UNMAP_VIEW.unwrap();
    original_fn(process_handle, base_address)
}

// NtUnmapViewOfSectionEx hook implementation
// Extended version of NtUnmapViewOfSection with flags parameter. Same logic as standard version.
unsafe extern "system" fn nt_unmap_view_of_section_ex_detour(
    process_handle: HANDLE,
    base_address: *mut c_void,
    flags: u32,
) -> NTSTATUS {
    let addr = base_address as usize;

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if this is our virtual mapping
    if state.mappings.contains_key(&addr) {
        println!("      → NtUnmapViewOfSectionEx hook: Virtual mapping detected");

        // Deregister exception range (to prevent handler from processing faults after unmapping)
        state.exception_ranges.remove(&addr);

        // Remove mapping tracking (the view is unmapped, but memory persists until NtClose)
        state.mappings.remove(&addr);

        println!("      → Address range deregistered from exception handler");
        println!("      → View unmapped (memory will be freed when section handle is closed)");

        drop(state_guard);

        return NTSTATUS(0); // STATUS_SUCCESS
    }
    drop(state_guard);

    // Not our virtual mapping - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_UNMAP_VIEW_EX.unwrap();
    original_fn(process_handle, base_address, flags)
}

static HOOKS_INITIALISED: AtomicBool = AtomicBool::new(false);

// Initialise hooks using retour
pub(crate) unsafe fn init_hooks() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Check if hooks are already initialised (idempotent for tests)
    if HOOKS_INITIALISED.load(Ordering::Acquire) {
        return Ok(());
    }

    // Initialise page size from system
    init_page_size();

    // Get ntdll.dll module handle
    let ntdll = GetModuleHandleW(w!("ntdll.dll")).expect("Failed to get ntdll.dll handle");

    // Resolve NT functions using the helper
    let nt_create_section: NtCreateSectionFn = resolve_nt_function(ntdll, "NtCreateSection");
    let nt_map_view: NtMapViewOfSectionFn = resolve_nt_function(ntdll, "NtMapViewOfSection");
    let nt_unmap_view: NtUnmapViewOfSectionFn = resolve_nt_function(ntdll, "NtUnmapViewOfSection");
    let nt_unmap_view_ex: NtUnmapViewOfSectionExFn =
        resolve_nt_function(ntdll, "NtUnmapViewOfSectionEx");
    let nt_close: NtCloseFn = resolve_nt_function(ntdll, "NtClose");

    // Create detours
    let detour_create = RawDetour::new(
        nt_create_section as *const () as *const _,
        nt_create_section_detour as *const () as *const _,
    )
    .expect("Failed to create NtCreateSection detour");

    let detour_map = RawDetour::new(
        nt_map_view as *const () as *const _,
        nt_map_view_of_section_detour as *const () as *const _,
    )
    .expect("Failed to create NtMapViewOfSection detour");

    let detour_unmap = RawDetour::new(
        nt_unmap_view as *const () as *const _,
        nt_unmap_view_of_section_detour as *const () as *const _,
    )
    .expect("Failed to create NtUnmapViewOfSection detour");

    let detour_unmap_ex = RawDetour::new(
        nt_unmap_view_ex as *const () as *const _,
        nt_unmap_view_of_section_ex_detour as *const () as *const _,
    )
    .expect("Failed to create NtUnmapViewOfSectionEx detour");

    let detour_close = RawDetour::new(
        nt_close as *const () as *const _,
        nt_close_detour as *const () as *const _,
    )
    .expect("Failed to create NtClose detour");

    // Get original function pointers (these point to the unhooked original functions)
    ORIGINAL_NT_CREATE_SECTION = Some(mem::transmute(detour_create.trampoline()));
    ORIGINAL_NT_MAP_VIEW = Some(mem::transmute(detour_map.trampoline()));
    ORIGINAL_NT_UNMAP_VIEW = Some(mem::transmute(detour_unmap.trampoline()));
    ORIGINAL_NT_UNMAP_VIEW_EX = Some(mem::transmute(detour_unmap_ex.trampoline()));
    ORIGINAL_NT_CLOSE = Some(mem::transmute(detour_close.trampoline()));

    // Enable detours
    detour_create
        .enable()
        .expect("Failed to enable NtCreateSection detour");
    detour_map
        .enable()
        .expect("Failed to enable NtMapViewOfSection detour");
    detour_unmap
        .enable()
        .expect("Failed to enable NtUnmapViewOfSection detour");
    detour_unmap_ex
        .enable()
        .expect("Failed to enable NtUnmapViewOfSectionEx detour");
    detour_close
        .enable()
        .expect("Failed to enable NtClose detour");

    // Store detours to prevent them from being dropped
    NT_CREATE_SECTION_DETOUR = Some(detour_create);
    NT_MAP_VIEW_DETOUR = Some(detour_map);
    NT_UNMAP_VIEW_DETOUR = Some(detour_unmap);
    NT_UNMAP_VIEW_EX_DETOUR = Some(detour_unmap_ex);
    NT_CLOSE_DETOUR = Some(detour_close);

    // Register VectoredExceptionHandler
    let handler_handle = AddVectoredExceptionHandler(1, Some(exception_handler));
    if handler_handle.is_null() {
        panic!("Failed to register VectoredExceptionHandler");
    }

    // Store handler handle for cleanup
    *EXCEPTION_HANDLER_HANDLE.lock().unwrap() = Some(HandlerHandle(handler_handle));

    println!("      ✓ VectoredExceptionHandler registered");

    // Mark hooks as initialised
    HOOKS_INITIALISED.store(true, Ordering::Release);

    Ok(())
}
