# Standard I/O (Layer 2)

## Layer 2 Hook Endpoints

!!! info "Layer 2: The 'What' Problem"

    Layer 2 handles virtual file data synthesis and read operations. It manages the actual
    file content that gets returned when virtual files are accessed.

### What It Does

Handles all operations on virtual files once they're opened. Layer 2 intercepts data operations (reads, seeks, etc.) and manages the lifecycle of virtual file handles. It works with `fileHandler` objects provided by Layer 3 extensions to synthesize file content on-the-fly.

Uses Layer 1 to make virtual files visible in directory searches and to handle path routing.

!!! tip "Read-Only Implementation Focus"

    Current documentation for Layer 2 only consists of reads. Support for file writes is not yet planned out, with respect to APIs, etc.

### Windows

#### Read Support

- **`NtCreateFile` & `NtOpenFile`**
    - Detect when a virtual file is being opened.
    - Look up the registered `fileHandler` for this path and initialize state for managing read operations.

- **`NtReadFile`**
    - Intercept file read operations.
    - If the file is virtual, delegate to the `fileHandler` to provide the actual data at the requested offset.

- **`NtReadFileScatter`**
    - Intercept scatter-gather read operations.
    - Reads file data into multiple non-contiguous buffers in a single operation.
    - If the file is virtual, delegate to the `fileHandler` and fill multiple buffers.
    - Provides API completeness alongside `NtReadFile`.

- **`NtSetInformationFile`**
    - Intercept handle update operations.
    - Track file pointer position updates (seek operations).
    - Virtual files need to maintain their own file pointer state.

- **`NtQueryInformationFile`**
    - Intercept file information queries.
    - Report the virtual file's size and attributes from the registered metadata.

- **`NtClose`**
    - Intercept file close operations.
    - Dispose of virtual file state (such as current read offset).
    - Free internal data structures for that virtual file instance.

- **`NtDuplicateObject`**
    - Intercept handle duplication operations.
    - Track duplicated handles that refer to virtual files.
    - Both original and duplicated handles share the same virtual file state (file position, etc.).
    - Use case is to reopen a handle with different permissions, but there's no reason for games to do this.
    - Included for completeness - never observed in actual game usage.

#### Write Support (Future)

When writable virtual files are implemented, additional APIs will be hooked:

- **`NtWriteFile`** & **`NtWriteFileGather`**
    - Write operations to virtual files.
    - Requires extending the `fileHandler` interface to support write callbacks.
    - `NtWriteFileGather` handles scatter-gather writes (complements `NtReadFileScatter`).

### Linux

!!! info "TODO: Document Linux syscalls for Layer 2"

    This section will document the Linux syscalls hooked by Layer 2.

## Summary

!!! success "Standard I/O Coverage"
    
    Standard I/O for Layer 2 requires hooking **7 core APIs** on Windows:
    
    - **File lifecycle**: `NtCreateFile`/`NtOpenFile` (detect virtual files), `NtClose` (cleanup), `NtDuplicateObject` (handle tracking)
    - **Data operations**: `NtReadFile` (standard reads), `NtReadFileScatter` (scatter-gather reads), `NtSetInformationFile` (seek/file pointer)
    - **Metadata queries**: `NtQueryInformationFile` (size/attributes)
    
    These cover 95-99% of games. More specialised I/O mechanisms have their own dedicated implementations:
    
    - **[Memory Mapped Files](Memory-Mapped-Files.md)** - Page fault handling for memory-mapped virtual files
    - **[DirectStorage & IoRing](DirectStorage.md)** - Asynchronous I/O for high-performance asset loading
