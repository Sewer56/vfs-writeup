// Hook implementations, state management, and initialisation

use crate::content;
use crate::nt_types::*;
use retour::RawDetour;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem;
use std::ptr;
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use windows::core::*;
use windows::Win32::Foundation::{HANDLE, NTSTATUS};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::SystemInformation::{GetSystemInfo, SYSTEM_INFO};

// Virtual file marker handle (synthetic pointer value representing virtual file)
const VIRTUAL_FILE_MARKER: usize = 0xDEADBEEF;

// System page size and file size (initialised from GetSystemInfo)
static mut PAGE_SIZE: usize = 0;
static mut FILE_SIZE: usize = 0; // 4 pages

// Initialise page size from system
unsafe fn init_page_size() {
    let mut system_info = SYSTEM_INFO::default();
    GetSystemInfo(&mut system_info);
    PAGE_SIZE = system_info.dwPageSize as usize;
    FILE_SIZE = PAGE_SIZE * 4; // 4 pages
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

// Tracking state for section handles and memory mappings
struct MappingState {
    // Maps section handle -> allocated memory base address
    sections: HashMap<usize, usize>,
    // Maps base address -> (size, section handle)
    mappings: HashMap<usize, (usize, usize)>,
}

// Use lazy initialisation for the global state
static MAPPING_STATE: Mutex<Option<MappingState>> = Mutex::new(None);

fn get_mapping_state() -> std::sync::MutexGuard<'static, Option<MappingState>> {
    let mut guard = MAPPING_STATE.lock().unwrap();
    if guard.is_none() {
        *guard = Some(MappingState {
            sections: HashMap::new(),
            mappings: HashMap::new(),
        });
    }
    guard
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

        println!("      → Allocating {} bytes with VirtualAlloc", FILE_SIZE);
        let memory = VirtualAlloc(
            Some(ptr::null()),
            FILE_SIZE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );

        if memory.is_null() {
            return NTSTATUS(-1073741823); // STATUS_UNSUCCESSFUL (0xC0000001)
        }

        println!("      → Memory allocated at {:?}", memory);
        println!("      → Populating memory with synthesised content...");

        // In a real VFS hook, we would read the virtual file and populate it into this buffer,
        // ideally via async I/O (e.g., IoRing) for performance.
        // When memory maps are opened with MapViewOfFile, we copy the file contents to this buffer.
        let buffer = slice::from_raw_parts_mut(memory as *mut u8, FILE_SIZE);
        content::synthesise(buffer, PAGE_SIZE);

        let num_pages = FILE_SIZE / PAGE_SIZE;
        println!("      → Content synthesised for {} pages", num_pages);

        // Use the memory address as section handle (simplified for demo)
        let section = HANDLE(memory);
        *section_handle = section;

        // Track section → memory mapping
        let mut state_guard = get_mapping_state();
        let state = state_guard.as_mut().unwrap();
        state.sections.insert(memory as usize, memory as usize);

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
//
// This hook is critical for proper resource cleanup.:
//
// - Section objects have independent lifetimes from their mapped views.
// - A section can have multiple views created and destroyed via NtMapViewOfSection/NtUnmapViewOfSection, but the section itself persists until the section
// handle is closed via NtClose.
// - Memory allocated during NtCreateSection must only be freed when the section handle is closed, not when individual views are unmapped.
//
// IMPORTANT: This implementation assumes views are unmapped before the section handle is closed
// (the normal Windows lifecycle). If a consumer closes the section handle without unmapping
// views first, we remove any remaining mapping entries to prevent stale references, though
// access to the freed memory would cause undefined behaviour. Applications should always
// unmap views before closing section handles.
unsafe extern "system" fn nt_close_detour(handle: HANDLE) -> NTSTATUS {
    let handle_value = handle.0 as usize;

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    // Check if this handle is a tracked section handle
    if let Some(memory_base) = state.sections.remove(&handle_value) {
        println!("      → NtClose hook: Virtual section handle detected");

        // Check if any mappings still exist for this section (shouldn't happen in normal usage)
        let mut mappings_to_remove = Vec::new();
        for (&base_addr, &(_size, section_handle)) in &state.mappings {
            if section_handle == handle_value {
                mappings_to_remove.push(base_addr);
            }
        }

        // Remove any remaining mapping entries to prevent stale references
        if !mappings_to_remove.is_empty() {
            println!(
                "      → Warning: {} mapping(s) still exist; removing to prevent stale references",
                mappings_to_remove.len()
            );
            for base_addr in mappings_to_remove {
                state.mappings.remove(&base_addr);
                println!("      → Removed mapping for base 0x{:X}", base_addr);
            }
        }

        drop(state_guard);

        // Free the allocated memory
        println!("      → Freeing allocated memory with VirtualFree");
        VirtualFree(memory_base as *mut c_void, 0, MEM_RELEASE).expect("VirtualFree failed");

        println!("      → Section cleaned up, memory freed");

        return NTSTATUS(0); // STATUS_SUCCESS
    }

    drop(state_guard);

    // Not our virtual section - call original function (unhooked implementation)
    let original_fn = ORIGINAL_NT_CLOSE.unwrap();
    original_fn(handle)
}

// NtMapViewOfSection hook implementation
// Here we return a pointer to the pre-populated memory as the user asks for a view of a slice of file.
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

    if let Some(&memory_base) = state.sections.get(&section_addr) {
        println!("      → NtMapViewOfSection hook: Virtual section detected");

        // Read the section_offset parameter and adjust base address accordingly.
        // In production, additional validation would be needed (alignment checks, bounds checking, etc.)
        let offset = if !section_offset.is_null() {
            unsafe { *section_offset }
        } else {
            0
        };

        // Apply offset to base address
        let offset_base = (memory_base as i64 + offset) as *mut c_void;
        *base_address = offset_base;

        // Adjust view size if offset is provided
        if !view_size.is_null() {
            let adjusted_size = FILE_SIZE.saturating_sub(offset as usize);
            *view_size = adjusted_size;
        }

        println!(
            "      → Mapping pre-populated memory at {:?} (offset: {} bytes)",
            *base_address, offset
        );

        drop(state_guard);

        // Track the mapping using the actual returned base address
        let mut state_guard = get_mapping_state();
        let state = state_guard.as_mut().unwrap();
        state.mappings.insert(
            offset_base as usize,
            (FILE_SIZE.saturating_sub(offset as usize), section_addr),
        );

        println!("      → Mapping tracked");

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
// This hook only removes the view mapping tracking. It does NOT free memory or remove the section
// from tracking. The section object persists independently of mapped views - multiple views can be
// created and destroyed from the same section. Memory deallocation happens in NtClose when the
// section handle itself is closed.
unsafe extern "system" fn nt_unmap_view_of_section_detour(
    process_handle: HANDLE,
    base_address: *mut c_void,
) -> NTSTATUS {
    let addr = base_address as usize;

    let mut state_guard = get_mapping_state();
    let state = state_guard.as_mut().unwrap();

    if let Some((_size, _section_handle)) = state.mappings.remove(&addr) {
        println!("      → NtUnmapViewOfSection hook: Virtual mapping detected");
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

    if let Some((_size, _section_handle)) = state.mappings.remove(&addr) {
        println!("      → NtUnmapViewOfSectionEx hook: Virtual mapping detected");
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

    // Mark hooks as initialised
    HOOKS_INITIALISED.store(true, Ordering::Release);

    Ok(())
}
