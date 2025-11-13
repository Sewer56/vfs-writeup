/// Synthesises content for a single page.
/// Used by page-fault handlers to populate pages on-demand.
///
/// In a real VFS implementation, this would read the corresponding page from
/// the virtual file, ideally via async I/O (e.g., IoRing) to avoid blocking.
pub fn synthesise_page(buffer: &mut [u8], page_num: usize) {
    let content = format!("Virtual file content page {}\n", page_num);
    let bytes = content.as_bytes();
    let copy_len = bytes.len().min(buffer.len());
    buffer[..copy_len].copy_from_slice(&bytes[..copy_len]);
}
