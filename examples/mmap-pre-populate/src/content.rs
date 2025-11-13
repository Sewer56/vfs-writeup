/// Synthesises virtual file content into the provided buffer.
/// Fills each page with a simple text pattern for demonstration purposes.
///
/// In a real VFS implementation, this would read actual mod file content
/// (ideally via async I/O like IoRing) and populate the buffer with that data.
pub fn synthesise(buffer: &mut [u8], page_size: usize) {
    let file_size = buffer.len();
    let num_pages = file_size / page_size;

    for page in 0..num_pages {
        let offset = page * page_size;
        let content = format!("Virtual file content page {}\n", page);
        let bytes = content.as_bytes();
        let copy_len = bytes.len().min(page_size);
        buffer[offset..offset + copy_len].copy_from_slice(&bytes[..copy_len]);
    }
}

/// Verifies that buffer content matches the expected synthesised pattern.
/// Prints verification messages for each page and returns an error if content doesn't match.
pub fn verify(buffer: &[u8], page_size: usize) -> Result<(), Box<dyn std::error::Error>> {
    let file_size = buffer.len();
    let num_pages = file_size / page_size;

    for page in 0..num_pages {
        let offset = page * page_size;
        let expected = format!("Virtual file content page {}\n", page);
        let actual_bytes = &buffer[offset..offset + expected.len()];
        let actual = std::str::from_utf8(actual_bytes)?;

        assert_eq!(actual, expected, "Content mismatch at page {}", page);
        println!("      ✓ Page {} verified: \"{}\"", page, actual.trim());
    }

    Ok(())
}
