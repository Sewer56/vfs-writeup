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
    // Maps section handle -> backing_storage_base_address
    // Each section has ONE backing storage allocation that all views share
    sections: HashMap<usize, usize>,
    // Maps view_base_address -> (view_size, section_handle, offset_into_section)
    // Tracks individual views that point into backing storage
    view_metadata: HashMap<usize, (usize, usize, i64)>,
    // Maps backing_storage_base -> (storage_size, section_handle)
    // Exception ranges registered per backing storage, not per view
    exception_ranges: HashMap<usize, (usize, usize)>,
}

// Use lazy initialisation for the global state
static MAPPING_STATE: Mutex<Option<MappingState>> = Mutex::new(None);

fn get_mapping_state() -> std::sync::MutexGuard<'static, Option<MappingState>> {
    let mut guard = MAPPING_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(MappingState {
            sections: HashMap::new(),
            view_metadata: HashMap::new(),
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

    // Check if fault address is in any tracked backing storage range
    let mut found_range: Option<(usize, usize, usize)> = None; // (backing_storage_base, size, section)
    for (&backing_storage_base, &(size, section)) in &state.exception_ranges {
        if fault_addr >= backing_storage_base && fault_addr < backing_storage_base + size {
            found_range = Some((backing_storage_base, size, section));
            break;
        }
    }

    let (backing_storage_base, _size, _section) = match found_range {
        Some(range) => range,
        None => return EXCEPTION_CONTINUE_SEARCH, // Not our virtual file
    };

    // Calculate which page was accessed relative to backing storage
    // Since all views share the same backing storage, the page number is calculated
    // relative to the backing storage base. This ensures commits are visible to all views.
    let page_num = (fault_addr - backing_storage_base) / PAGE_SIZE;
    let page_start_addr = backing_storage_base + (page_num * PAGE_SIZE);

    // Drop the lock before calling VirtualAlloc to avoid holding it during system call
    drop(state_guard);

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
    content::synthesise_page(buffer, page_num);

    println!(
        "      → Page fault handler: Committed and populated file page {} at address 0x{:X}",
        page_num, page_start_addr
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

        // CRITICAL: Allocate backing storage ONCE for the ENTIRE file (all views will reference this)
        // Use MEM_RESERVE to reserve address space without committing physical memory
        // Pages will be committed on-demand by the exception handler
        println!(
            "      → Allocating backing storage ({} bytes) with VirtualAlloc(MEM_RESERVE)",
            FILE_SIZE
        );
        let backing_storage = VirtualAlloc(None, FILE_SIZE, MEM_RESERVE, PAGE_NOACCESS);

        if backing_storage.is_null() {
            return NTSTATUS(-1073741823); // STATUS_UNSUCCESSFUL
        }

        println!("      → Backing storage allocated at {:?}", backing_storage);

        // Create a synthetic section handle (use backing storage address for simplicity)
        let section = HANDLE(backing_storage);
        *section_handle = section;

        // Track section -> backing storage mapping
        let mut state_guard = get_mapping_state();
        let state = state_guard.as_mut().unwrap();
        let section_addr = section.0 as usize;
        state
            .sections
            .insert(section_addr, backing_storage as usize);

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
    if let Some(backing_storage) = state.sections.remove(&handle_value) {
        println!("      → NtClose hook: Virtual section handle detected");

        // Find all views that reference this section (handles both normal and abnormal lifecycle)
        let mut views_to_remove = Vec::new();
        for (&view_addr, &(_size, section_handle, _offset)) in &state.view_metadata {
            if section_handle == handle_value {
                views_to_remove.push(view_addr);
            }
        }

        // Warn if views still exist (indicates abnormal lifecycle - should unmap before closing)
        if !views_to_remove.is_empty() {
            println!(
                "      → Warning: {} view(s) still exist; cleaning up to prevent stale references",
                views_to_remove.len()
            );
        }

        // Remove all view metadata for this section
        for view_addr in views_to_remove {
            state.view_metadata.remove(&view_addr);
            println!(
                "      → Removed view tracking for address 0x{:X}",
                view_addr
            );
        }

        // Remove exception range for the backing storage
        state.exception_ranges.remove(&backing_storage);
        println!(
            "      → Deregistered exception range for backing storage 0x{:X}",
            backing_storage
        );

        drop(state_guard);

        // CRITICAL: Free the ONE backing storage allocation (works for all views that referenced it)
        VirtualFree(backing_storage as *mut c_void, 0, MEM_RELEASE).expect("VirtualFree failed");
        println!("      → Freed backing storage at 0x{:X}", backing_storage);
        println!("      → Section cleaned up (single backing storage freed)");

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

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if this is our virtual section and retrieve backing storage
    let backing_storage = match state.sections.get(&section_addr) {
        Some(&bs) => bs,
        None => {
            drop(state_guard);
            // Not our virtual section - call original function (unhooked implementation)
            let original_fn = ORIGINAL_NT_MAP_VIEW.unwrap();
            return original_fn(
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
            );
        }
    };

    println!("      → NtMapViewOfSection hook: Virtual section detected");

    // PROOF OF CONCEPT: Basic offset support demonstrated.
    // Read the section_offset parameter. In production, additional validation would be needed
    // (alignment checks, bounds checking, offset + view_size <= file_size, etc.)
    let offset = if !section_offset.is_null() {
        unsafe { *section_offset }
    } else {
        0
    };

    // CRITICAL: Return pointer INTO existing backing storage (no new allocation)
    // Calculate view address: backing_storage + offset
    let view_address = (backing_storage as i64 + offset) as *mut c_void;
    let adjusted_size = FILE_SIZE.saturating_sub(offset as usize);

    println!(
        "      → Returning pointer into backing storage: 0x{:X} (backing_storage: 0x{:X} + offset: {} bytes)",
        view_address as usize, backing_storage, offset
    );

    // Set output parameters
    *base_address = view_address;
    if !view_size.is_null() {
        *view_size = adjusted_size;
    }

    // Track view metadata for cleanup
    let view_addr = view_address as usize;
    state
        .view_metadata
        .insert(view_addr, (adjusted_size, section_addr, offset));

    // Register exception range for backing storage (only if not already registered)
    // Multiple views can share the same backing storage
    if let std::collections::hash_map::Entry::Vacant(e) =
        state.exception_ranges.entry(backing_storage)
    {
        e.insert((FILE_SIZE, section_addr));
        println!(
            "      → Registered backing storage exception range: [0x{:X}, 0x{:X})",
            backing_storage,
            backing_storage + FILE_SIZE
        );
    } else {
        println!(
            "      → Backing storage exception range already registered (shared by multiple views)"
        );
    }

    println!("      → View tracked (lazy commitment enabled)");

    NTSTATUS(0) // STATUS_SUCCESS
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
    if let Some((_view_size, section_handle, _offset)) = state.view_metadata.remove(&addr) {
        println!("      → NtUnmapViewOfSection hook: Virtual mapping detected");

        // Check if any other views still reference the same section
        let other_views_exist = state
            .view_metadata
            .values()
            .any(|(_, sh, _)| *sh == section_handle);

        // Only deregister exception range if no other views exist for this section
        if !other_views_exist {
            // Find backing storage for this section to deregister exception range
            if let Some(&backing_storage) = state.sections.get(&section_handle) {
                state.exception_ranges.remove(&backing_storage);
                println!("      → Deregistered exception range (last view for this section)");
            }
        } else {
            println!("      → Exception range kept (other views still reference this section)");
        }

        println!("      → View unmapped (backing storage persists until section handle is closed)");

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
    if let Some((_view_size, section_handle, _offset)) = state.view_metadata.remove(&addr) {
        println!("      → NtUnmapViewOfSectionEx hook: Virtual mapping detected");

        // Check if any other views still reference the same section
        let other_views_exist = state
            .view_metadata
            .values()
            .any(|(_, sh, _)| *sh == section_handle);

        // Only deregister exception range if no other views exist for this section
        if !other_views_exist {
            // Find backing storage for this section to deregister exception range
            if let Some(&backing_storage) = state.sections.get(&section_handle) {
                state.exception_ranges.remove(&backing_storage);
                println!("      → Deregistered exception range (last view for this section)");
            }
        } else {
            println!("      → Exception range kept (other views still reference this section)");
        }

        println!("      → View unmapped (backing storage persists until section handle is closed)");

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
