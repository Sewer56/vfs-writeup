#![allow(clippy::missing_transmute_annotations)]
#![allow(static_mut_refs)]

mod content;
mod hooks;
mod nt_types;

use std::ffi::c_void;
use std::slice;
use windows::core::*;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, MapViewOfFileEx, UnmapViewOfFile, FILE_MAP_READ,
    PAGE_READONLY,
};

fn main() {
    if let Err(e) = run_demo() {
        eprintln!("\nError: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Virtual file marker handle (synthetic pointer value representing virtual file)
    println!("\n=== Pre-Population Strategy Demo (PROOF OF CONCEPT) ===\n");
    println!("This demo demonstrates the five-hook lifecycle for memory-mapped virtual files.");
    println!("Real API hooking is performed using the retour crate.\n");
    println!("NOTE: This is a simplified PoC. Offset support is demonstrated but production code");
    println!(
        "      would include additional validation, alignment checks, and edge case handling.\n"
    );

    println!("[1/7] Installing NT-level hooks...");
    unsafe {
        hooks::init_hooks()?;
    }
    println!("      ✓ NtCreateSection hook installed");
    println!("      ✓ NtMapViewOfSection hook installed");
    println!("      ✓ NtUnmapViewOfSection hook installed");
    println!("      ✓ NtUnmapViewOfSectionEx hook installed");
    println!("      ✓ NtClose hook installed");
    println!("      → Win32 APIs (UnmapViewOfFile, CloseHandle) internally call hooked NT APIs\n");

    println!("[2/7] Creating virtual file marker...");
    let marker_handle = HANDLE(0xDEADBEEF as *mut c_void);
    println!("      ✓ Marker created: 0x{:X}\n", 0xDEADBEEFusize);

    // Get page size for calculations (4 pages file)
    let page_size = unsafe {
        let mut system_info = windows::Win32::System::SystemInformation::SYSTEM_INFO::default();
        windows::Win32::System::SystemInformation::GetSystemInfo(&mut system_info);
        system_info.dwPageSize as usize
    };
    let file_size = page_size * 4;

    // CreateFileMappingW internally calls NtCreateSection, which our hook intercepts.
    // Our hook detects the virtual file 'marker', allocates memory, and populates it with virtual file content.
    // This is the "pre-population" strategy (for small files): all content synthesised upfront during section creation.
    println!("[3/7] Calling CreateFileMappingW..."); // Invokes NtCreateSection -> nt_create_section_detour
    let section_handle = unsafe {
        CreateFileMappingW(
            marker_handle,
            None,
            PAGE_READONLY,
            0,
            file_size as u32,
            PCWSTR::null(),
        )?
    };
    println!("      ✓ Section created: {:?}\n", section_handle);

    // MapViewOfFile internally calls NtMapViewOfSection, which our hook intercepts.
    // Our hook detects the virtual section and returns a pointer to the already-allocated, pre-populated memory.
    // Application can read immediately with zero page faults—all content is already in physical memory.
    println!("[4/7] Calling MapViewOfFile..."); // Invokes NtMapViewOfSection -> nt_map_view_of_section_detour
    let base_address = unsafe { MapViewOfFile(section_handle, FILE_MAP_READ, 0, 0, file_size) };

    if base_address.Value.is_null() {
        return Err("MapViewOfFile failed".into());
    }

    println!("      ✓ Mapped at address: {:?}", base_address.Value);
    println!("      ✓ View size: {} bytes\n", file_size);

    println!("[5/7] Reading and verifying content...");
    println!("      → Application accesses memory directly (no page faults!)");
    unsafe {
        let buffer = slice::from_raw_parts(base_address.Value as *const u8, file_size);
        content::verify(buffer, page_size)?;
    }
    println!();

    println!("[5b/7] Testing offset mapping...");
    println!("      → Mapping same section at offset (PAGE_SIZE)...");

    unsafe {
        // Split 64-bit offset into high/low DWORDs for MapViewOfFileEx
        let offset_value = page_size as u64;
        let offset_high = (offset_value >> 32) as u32;
        let offset_low = (offset_value & 0xFFFFFFFF) as u32;
        let adjusted_size = file_size - page_size;

        // MapViewOfFileEx internally calls NtMapViewOfSection, which our hook intercepts
        let offset_base = MapViewOfFileEx(
            section_handle,
            FILE_MAP_READ,
            offset_high,
            offset_low,
            adjusted_size,
            None, // let OS choose address
        );

        if offset_base.Value.is_null() {
            return Err("MapViewOfFileEx failed for offset mapping".into());
        }

        println!("      ✓ Offset mapping created at {:?}", offset_base.Value);
        println!(
            "      ✓ View size: {} bytes (file_size - offset)",
            adjusted_size
        );

        // Verify offset mapping shows page 1 content (not page 0)
        let offset_buffer = slice::from_raw_parts(offset_base.Value as *const u8, adjusted_size);
        let expected_page1 = "Virtual file content page 1\n";
        let actual_bytes = &offset_buffer[0..expected_page1.len()];
        let actual = std::str::from_utf8(actual_bytes)?;

        assert_eq!(actual, expected_page1, "Offset content mismatch");
        println!("      ✓ Offset content verified: \"{}\"", actual.trim());
        println!("      → Content at offset correctly shows page 1 (not page 0)\n");

        // Unmap offset view (UnmapViewOfFile internally calls NtUnmapViewOfSection)
        UnmapViewOfFile(offset_base)?;
        println!("      ✓ Offset view unmapped\n");
    }

    // UnmapViewOfFile internally calls NtUnmapViewOfSection, which our hook intercepts
    println!("[6/7] Calling UnmapViewOfFile..."); // Invokes NtUnmapViewOfSection -> nt_unmap_view_of_section_detour
    unsafe {
        println!("      → Unmapping view at {:?}", base_address.Value);
        UnmapViewOfFile(base_address)?;
        println!("      ✓ View unmapped successfully\n");
    }

    // CloseHandle internally calls NtClose, which our hook intercepts
    println!("[7/7] Closing section handle..."); // Invokes NtClose -> nt_close_detour
    unsafe {
        CloseHandle(section_handle)?;
        println!("      ✓ Section handle closed successfully\n");
    }

    Ok(())
}
