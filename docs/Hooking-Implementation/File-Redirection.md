# File Redirection (Layer 1)

!!! info "Layer 1: Path Redirection and Virtual File Visibility"
    
    Layer 1 handles the "where" problem - making virtual files visible in directory listings and redirecting file paths. This layer intercepts OS file APIs to inject virtual file information without modifying the actual filesystem.

!!! note "Prior Art"
    
    If this page doesn't seem overly detailed, it's because I've already done all of this years ago in [Reloaded Universal Redirector](https://github.com/Reloaded-Project/reloaded.universal.redirector/tree/rewrite-usvfs-read-features/Reloaded.Universal.Redirector), and so has [usvfs](https://github.com/ModOrganizer2/usvfs). This just has a few missing bits from before.

## Layer 1 Hook Endpoints

!!! info "Layer 1: The 'Where' Problem"

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

## That's All for Layer 1

!!! success "Complete API Coverage"
    
    That's all that's needed for Layer 1. Technologies like [Memory Mapped Files](Virtual-Files/Memory-Mapped-Files.md) and [DirectStorage](Virtual-Files/DirectStorage.md) don't require additional Layer 1 hooks - they're handled entirely by Layer 2.
    
    Path redirection is relatively straightforward: just a lot of enums and options to handle on each Windows API.
