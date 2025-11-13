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
    CreateFileMappingW, MapViewOfFileEx, UnmapViewOfFile, FILE_MAP_READ,
    MEMORY_MAPPED_VIEW_ADDRESS, PAGE_READONLY,
};

fn main() {
    if let Err(e) = run_demo() {
        eprintln!("\nError: {}", e);
        std::process::exit(1);
    }
}

fn run_demo() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Virtual file marker handle (synthetic pointer value representing virtual file)
    println!("\n=== Page Fault Emulation Strategy Demo (PROOF OF CONCEPT) ===\n");
    println!("This demo demonstrates the five-hook lifecycle with lazy page commitment.");
    println!("Only accessed pages will be committed (sparse memory usage).\n");
    println!("NOTE: This is a simplified PoC. Offset support is demonstrated but production code");
    println!(
        "      would include additional validation, alignment checks, and edge case handling.\n"
    );

    println!("[1/8] Installing NT-level hooks and exception handler...");
    unsafe {
        hooks::init_hooks()?;
    }
    println!("      ✓ NtCreateSection hook installed");
    println!("      ✓ NtMapViewOfSection hook installed");
    println!("      ✓ NtUnmapViewOfSection hook installed");
    println!("      ✓ NtUnmapViewOfSectionEx hook installed");
    println!("      ✓ NtClose hook installed");
    println!("      → Win32 APIs (UnmapViewOfFile, CloseHandle) internally call hooked NT APIs\n");

    println!("[2/8] Creating virtual file marker...");
    let marker_handle = HANDLE(0xDEADBEEF as *mut c_void);
    println!("      ✓ Marker created: 0x{:X}\n", 0xDEADBEEFusize);

    // Get page size for offset calculations (6 pages file)
    let page_size = unsafe {
        let mut system_info = windows::Win32::System::SystemInformation::SYSTEM_INFO::default();
        windows::Win32::System::SystemInformation::GetSystemInfo(&mut system_info);
        system_info.dwPageSize as usize
    };
    let file_size = page_size * 6;

    // CreateFileMappingW internally calls NtCreateSection, which our hook intercepts.
    // Our hook detects the marker and creates a synthetic section handle (no memory allocation yet).
    // This is the "page-fault" strategy (for large files): memory allocated and populated on-demand during access.
    println!("[3/8] Calling CreateFileMappingW..."); // Invokes NtCreateSection -> nt_create_section_detour
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

    println!("[4/8] Calling MapViewOfFileEx with offset...");
    println!("      → Mapping at offset PAGE_SIZE to demonstrate offset support");

    // Map at offset PAGE_SIZE to test offset functionality
    let offset_value: u64 = page_size as u64;
    let adjusted_size = file_size - page_size;

    unsafe {
        // Split 64-bit offset into high/low DWORDs for MapViewOfFileEx
        let offset_high = (offset_value >> 32) as u32;
        let offset_low = (offset_value & 0xFFFFFFFF) as u32;

        // MapViewOfFileEx internally calls NtMapViewOfSection, which our hook intercepts
        let base_ptr = MapViewOfFileEx(
            section_handle,
            FILE_MAP_READ,
            offset_high,
            offset_low,
            adjusted_size,
            None, // let OS choose address
        );

        if base_ptr.Value.is_null() {
            return Err("MapViewOfFileEx failed".into());
        }

        println!("      ✓ Mapped at address: {:?}", base_ptr.Value);
        println!(
            "      ✓ View size: {} bytes (MEM_RESERVE, not committed yet)",
            adjusted_size
        );
        println!(
            "      ✓ Offset: {} bytes (starting at file page 1)\n",
            offset_value
        );

        let base_address = base_ptr.Value;
        let adjusted_file_size = adjusted_size;

        println!("[5/8] Accessing sparse pages with offset...");
        println!("      → Note: View page 0 corresponds to file page 1 (due to offset)");
        println!("      → Reading view page 0 (file page 1, triggers page fault)...");
        // This access triggers the VectoredExceptionHandler (exception_handler in hooks.rs)
        // which commits the page on-demand and populates it with synthesised content

        let buffer = slice::from_raw_parts(base_address as *const u8, adjusted_file_size);

        // Access view page 0, which should show file page 1 content due to offset
        let expected_page1 = "Virtual file content page 1\n";
        let actual_page1_bytes = &buffer[0..expected_page1.len()];
        let actual_page1 = std::str::from_utf8(actual_page1_bytes)?;

        assert_eq!(
            actual_page1, expected_page1,
            "Content mismatch at view page 0 (file page 1)"
        );
        println!(
            "      ✓ View page 0 (file page 1) verified: \"{}\"",
            actual_page1.trim()
        );

        println!("      → Reading view page 4 (file page 5, triggers page fault)...");
        // This access triggers the VectoredExceptionHandler (exception_handler in hooks.rs)

        // Access view page 4, which should show file page 5 content due to offset
        let view_offset_page4 = 4 * page_size;
        let expected_page5 = "Virtual file content page 5\n";
        let actual_page5_bytes =
            &buffer[view_offset_page4..view_offset_page4 + expected_page5.len()];
        let actual_page5 = std::str::from_utf8(actual_page5_bytes)?;

        assert_eq!(
            actual_page5, expected_page5,
            "Content mismatch at view page 4 (file page 5)"
        );
        println!(
            "      ✓ View page 4 (file page 5) verified: \"{}\"",
            actual_page5.trim()
        );

        println!("\n[6/8] Verifying sparse commitment with offset...");
        println!(
            "      → Total pages in view: {}",
            adjusted_file_size / page_size
        );
        println!("      → Expected committed file pages: [1, 5]");
        println!("      ✓ Only accessed pages committed (lazy commitment with offset proven)\n");

        // Store base address for unmapping
        let base_address_for_unmap = base_address;

        // UnmapViewOfFile internally calls NtUnmapViewOfSection, which our hook intercepts
        println!("[7/8] Calling UnmapViewOfFile..."); // Invokes NtUnmapViewOfSection -> nt_unmap_view_of_section_detour
        println!("      → Unmapping view at {:?}", base_address_for_unmap);

        let base_address_value = MEMORY_MAPPED_VIEW_ADDRESS {
            Value: base_address_for_unmap,
        };
        UnmapViewOfFile(base_address_value)?;

        println!("      ✓ View unmapped successfully\n");
    }

    // CloseHandle internally calls NtClose, which our hook intercepts
    println!("[8/8] Closing section handle..."); // Invokes NtClose -> nt_close_detour
    unsafe {
        CloseHandle(section_handle)?;
        println!("      ✓ Section handle closed successfully\n");
    }

    Ok(())
}
