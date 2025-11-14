

## Hooking Strategy

!!! info "Implementation approach for intercepting file I/O"

    This section explains how we hook file operations on each platform, based on the
    filesystem architectures described above.

### Windows

!!! question "Why hook ntdll.dll?"

    As explained in the [Windows Filesystem Architecture](#windows) section, `ntdll.dll` is the lowest-level user-mode library on Windows, sitting directly above the kernel. Since Windows syscall numbers are unstable between versions, all user-mode software must use `ntdll.dll` APIs.
    
    By hooking at the ntdll level, we intercept **all** file operations from any software on Windows with a single interception point.

!!! tip "Wine Compatibility"

    This single interception point also works with Wine on Linux, since Wine aims to implement Win32 as closely as possible- including the relationship between `kernel32.dll` and `ntdll.dll`.

### Linux

!!! info "Three hooking approaches for Linux"

    There are three options for hooking file I/O on native Linux:
    
    1. **Hook `libc`** (e.g., `glibc`) - Works for ~99% of programs, but misses programs that syscall directly (e.g., Zig programs, statically-linked `musl` binaries).
    
    2. **Directly patch syscalls** - Disassemble every loaded library (`.so` file) to find syscall instructions and patch them to jump to our hook functions. Provides 100% coverage.
    
    3. **Use `ptrace`** - Intercept syscalls at the kernel boundary. This is the 'officially' supported solution, but has notable performance overhead. Not every distro allows `ptrace` out of the box. This is what Snap uses on Ubuntu, contributing to slow startup times.

!!! success "Our approach: Syscall patching (Option 2)"

    The optimal solution for our requirements (performance + coverage) is direct syscall patching.
    
    This requires running a disassembler on every loaded library to find syscall instructions and patch them with jumps to our hook functions.
    
    **Implementation notes:**
    
    - Needs to be implemented per architecture (x86_64, AArch64, etc.)
    - After the first architecture is implemented (x86_64), additional architectures take approximately one day each
    - Requires low-level knowledge but is straightforward

## Implementation Strategy

!!! tip "Read-Only Implementation Focus"

    The initial implementation focuses on **read-only access** and **path redirection**. This covers >99% of games and game mods, including e.g. switching folders for save file redirection.
    
    **Full write support** is only required by a certain few, uncommon modding tools that are built to operate on a pre-modded game folder (*cough* Skyrim *cough*). Including moving files, directory deletion, etc. will require planning, as current requirements are unclear and undefined (e.g. 'do we put in game folder? overrides folder? mod folder?' etc.). That would require discussion with additional people.

The hook endpoints below are split into "Read Support" and "Write Support (Future)" sections to clearly distinguish implementation priorities.

## Layer 1 Hook Endpoints

!!! info "Reminder: [Layer 1 deals with the 'where' problem](#layer-1-virtual-filesystem)"

    This section documents the specific APIs hooked by Layer 1 for each platform.
    
    Layer 1 injects either information of redirected/joined files from other folders, or injects information supplied by Layer 2 for virtual files.

### Windows

#### Read Support

- **`NtCreateFile`** & **`NtOpenFile`**
    - Intercept file creation/open operations. 
      - Check if path should be redirected when creating new files.
      - Substitute with target path before calling original API.
    - For 'virtual files', spoof creation to succeed without touching disk.

- **`NtQueryDirectoryFile`** & **`NtQueryDirectoryFileEx`**
    - Inject virtual files into directory search results. 
      - When application searches a directory, inject registered virtual files into the result set.
    - Uses semaphore to avoid recursion between the two APIs on Windows 10+.

- **`NtQueryAttributesFile`** & **`NtQueryFullAttributesFile`**
    - Return metadata for virtual/redirected files.
    - Path-based queries without opening the file.

- **`NtQueryInformationByName`**
    - Path-based file information query (Windows 10 1703+).
    - Modern equivalent to `NtQueryAttributesFile` with extended capabilities.
    - Returns file size, timestamps, and attributes without opening the file.
    - Used by [`GetFileInformationByName`](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getfileinformationbyname) (new in Windows 11 24H2; considered prerelease).

- **`NtQueryObject`**
    - Query object information including the final path name.
    - Needed for symlink resolution and path queries on file handles.
    - Used by `GetFinalPathNameByHandleW` to retrieve the final path for a file handle.
    - Must return the virtual/redirected path to maintain the illusion of file location.

- **`NtClose`** - Track when file handles are closed. Used for internal handle lifecycle management.

!!! warning "Wine Compatibility"

    `NtQueryInformationByName` and `NtQueryDirectoryFileEx` are not implemented in Wine (as of November 10, 2025).

#### Write Support (Future)

When write support is implemented, additional APIs will be hooked:

- **`NtDeleteFile`** - Handle deletion operations on virtual/redirected files. Intercept deletion requests and handle appropriately.

- **`NtNotifyChangeDirectoryFile`** & **`NtNotifyChangeDirectoryFileEx`**
    - Directory change monitoring for dynamic file modifications.
    - Only needed when dynamically adding/removing virtual files at runtime.
    - Only needed when supporting write operations that other applications need to observe.
    - Note: `NtNotifyChangeDirectoryFile` wraps `NtNotifyChangeDirectoryFileEx` on modern Windows.

### Linux

!!! info "TODO: Document Linux syscalls for Layer 1"

    This section will document the Linux syscalls hooked by Layer 1.

## Layer 2 Hook Endpoints

!!! info "Reminder: [Layer 2 deals with the 'what' problem](#layer-2-virtual-file-framework)"

    Layer 2 handles virtual file data synthesis and read operations. It manages the actual
    file content that gets returned when virtual files are accessed.

### What It Does

Handles all operations on virtual files once they're opened. Layer 2 intercepts data operations (reads, seeks, etc.) and manages the lifecycle of virtual file handles. It works with `fileHandler` objects provided by Layer 3 extensions to synthesize file content on-the-fly.

Uses Layer 1 to make virtual files visible in directory searches and to handle path routing.

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

**In the long term**, for virtual files, we will need to also handle memory-mapped file (mmap) operations:

- **`NtCreateSection`** & **`NtCreateSectionEx`**
    - Track creation of file memory maps.
    - Populate the section from the `fileHandler` and map it into the process.
    - `NtCreateSectionEx` is the modern extended variant used by `CreateFileMapping2` and `CreateFileMappingFromApp`.

- **`NtMapViewOfSection`** & **`NtUnmapViewOfSection`**
    - Pre-populate memory maps, or change permissions and add exception handler to emulate page faults.
    - Applications may map the same section multiple times with different offsets, sizes, or protection flags.
    - `NtUnmapViewOfSection` is needed for cleanup and reference counting.

For small mapping regions (<128K), we can pre-populate, otherwise we page fault emulate.

!!! note "Memory mapping is rare in games"

    Very few games actually use memory-mapped I/O.
    
    I (Sewer) have not yet encountered a game/engine that uses memory mapping in practice (out of a sample of ~20 games).
    
    Since many games traditionally shipped on optical discs (until very recently), memory mapping was not a viable option for asset loading. 
    
    In addition, many consider the CPU overhead of handling page faults to not be worthwhile compared to the overhead of a second copy.
    
    These APIs are included for completeness.

#### Write Support (Future)

When writable virtual files are implemented, additional APIs will be hooked:

- **`NtWriteFile`** & **`NtWriteFileGather`**
    - Write operations to virtual files.
    - Requires extending the `fileHandler` interface to support write callbacks.
    - `NtWriteFileGather` handles scatter-gather writes (complements `NtReadFileScatter`).

### Linux

!!! info "TODO: Document Linux syscalls for Layer 2"

    This section will document the Linux syscalls hooked by Layer 2.