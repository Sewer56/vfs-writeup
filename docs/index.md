!!! info "About This Documentation"
	Forked and improved from the [Reloaded3 Specification](https://reloaded-project.github.io/Reloaded-III/Mods/Essentials/Virtual-FileSystem/About.html). May upstream back to R3 docs, or reuse in actual project.

# User Space Virtual Filesystems

This wiki describes the implementation details of a **two-layer architecture** for implementing a Virtual FileSystem suitable for modding games.

## Implementation Overview

The architecture separates concerns into two distinct layers, each with specific responsibilities:

!!! warning "Logical Layer Separation"

    These layers represent logical ***architectural boundaries***, not necessarily separate binaries or libraries.
    
    Simply, don't mix Layer 1 & 2 code, or you'll make a mess of it; put them in separate files
    or projects!

    Keep Layer 1 code focused on path operations and Layer 2 code focused on data synthesis.

### Layer 1: Virtual FileSystem

!!! info "This handles the 'where' problem"

    Layer 1 focuses on path redirection and virtual file visibility.

- **Path redirection** - Make the OS open file B when the application asks for file A
- **Virtual file injection** - Make files appear in directory listings even if they don't exist on disk
- **Metadata spoofing** - Return correct file attributes and sizes for redirected/virtual files

Layer 1 operates at the path/metadata level. It doesn't care about file contents, only about routing requests to the right location and making virtual files visible.

i.e. **Layer 1** hooks path/metadata operations (`NtOpenFile`, `NtQueryDirectoryFile`, `NtQueryAttributesFile`, etc.)

**[Complete Implementation Details →](Virtual-FileSystem/About.md)**

#### Layer 1 Key APIs

- **`AddRedirect(sourcePath, targetPath)`** - Redirect individual file paths.

- **`RemoveRedirect(handle)`** - Remove an individual redirect.

- **`AddRedirectFolder(sourceFolder, targetFolder)`** - Overlay entire folder structure. Files in targetFolder appear in sourceFolder.

- **`RemoveRedirectFolder(handle)`** - Remove a folder overlay.

And this private API:

- **`RegisterVirtualFile(path, metadata)`** - Make a virtual file visible in directory searches. Layer 2 calls this to register virtual files so they appear when games search directories.

- **`UnregisterVirtualFile(handle)`** - Remove a virtual file from directory search results.

### Layer 2: Virtual File Framework

!!! info "This handles the 'what' problem"

    Layer 2 is all about handling access to the files returned by Layer 1.
    Providing custom data as it's read, keeping track of file seeks, etc.

Layer 2 deals with all of the events that happen to a ***virtual*** file once
it's been opened. The extensions in Layer 3 can create virtual files through Layer 2's API,
and Layer 2 will handle all of the interactions with the operating system for that file.

***Layer 2 is an abstraction*** for the extensions in Layer 3.

i.e. **Layer 2** hooks data operations (`NtReadFile`, `NtSetInformationFile`, etc.) and calls Layer 1's `RegisterVirtualFile()` API

#### Layer 2 Key APIs

These are public versions of Layer 1's private APIs:

- **`RegisterVirtualFile(path, metadata, fileHandler)`** - Allows extensions to create virtual files that Layer 1 will make visible in directory searches. The `metadata` is immutable metadata about file (e.g. size), the `fileHandler` parameter is an object that implements methods for handling read operations.

- **`UnregisterVirtualFile(handle)`** - Removes a virtual file registered earlier.

### Layer 3: Extensions

!!! info "About Layer 3"

    Layer 3 forms the extensions that are built on top of Layers 1 & 2.
    Those can be thought of as 'plugins' that implement specific behaviours.

#### Example Extension: Archive Emulation Framework

!!! info "Originally part of Reloaded-II's `FileEmulationFramework`"

The **Archive Emulation Framework** allows injecting files into game archives without writing code, using supported archive emulators that are built on top of the framework.

It provides a declarative way to modify archive contents by simply placing files in specific folder structures.

##### Route System

Files are identified by their full path including archive nesting:

```
<GameFolder>/English/Sound.afs/00000.adx
                    └ Archive ┘└ File Inside ┘
```

Emulators match against routes using partial path matching. A route pattern like `Sound.afs/00000.adx` matches any path ending with that pattern. More specific patterns take precedence:

- `English/Sound.afs/00000.adx` matches only files in the English folder
- `Sound.afs/00000.adx` matches Sound.afs in any folder

This allows precise targeting of files inside archives without requiring full absolute paths.

##### Emulator Chaining

Emulators can operate on files inside other emulated files. For example:

```
FileEmulationFramework/
  ONE/
    textures.one/
      textures.txd          ← Inject textures.txd into textures.one archive
  TXD/
    textures.txd/
      texture_001.dds       ← Inject texture into textures.txd (which is inside .one)
```

When the game opens `textures.one`, the ONE emulator emulates it. When it reads `textures.txd` from inside, the TXD emulator emulates that. Routes compose naturally through the path hierarchy. i.e. The system works recursively.

!!! info "TODO: Document Emulator Chaining/Nesting without Dummy Files"

    There's some solution in use today, I forgot the details.

#### Example Extension: Nx2VFS

**Nx2VFS** is a practical implementation that uses Layer 2 to provide an archive-backed filesystem. Games see normal files on disk, but they're actually backed by compressed `.nx2` archives containing multiple files.

In this case, Nx2VFS would call Layer 2's `RegisterVirtualFile()` for each file contained in the original `.nx2` archive. And a `fileHandler` implementation to fill in the actual data.

**How Nx2VFS Works:**

```
Archive on Disk:                    What Games See:
┌──────────────────┐                ┌──────────────────────────┐
│  game.nx2        │  Nx2VFS        │  game/                   │
│  ├─ player.model │ ──────────────▶│  ├─ player.model         │
│  ├─ enemy.model  │  registers     │  ├─ enemy.model          │
│  └─ level.map    │  each file     │  └─ level.map            │
└──────────────────┘                └──────────────────────────┘
    (compressed)                    (appear as normal files)
```

**Creating Virtual Files:**

```
┌─────────────────────────────────────────────────────────────────┐
│ Nx2VFS Extension (Layer 3)                                      │
└─────────────────────────────────────────────────────────────────┘
                          │
                          │ 1. Parse game.nx2
                          │    Discover: player.model, enemy.model, level.map
                          ▼
        ┌─────────────────────────────────────────┐
        │ For each file in archive:               │
        │                                         │
        │  RegisterVirtualFile(                   │
        │    path:        "game/player.model"     │
        │    metadata:    {size: 1024, ...}       │
        │    fileHandler: Nx2FileHandler          │
        │  )                                      │
        └─────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 2: Virtual File Framework                                 │
│ • Stores fileHandler reference                                  │
│ • Calls Layer 1's RegisterVirtualFile(path, metadata)           │
└─────────────────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────────┐
│ Layer 1: Virtual FileSystem                                     │
│ • Adds "game/player.model" to virtual file registry             │
│ • Files now appear in directory listings                        │
└─────────────────────────────────────────────────────────────────┘
```

**Resolving Reads:**

```
When game opens 'game/player.model':
  1. Layer 2 intercepts the read (NtReadFile)
  2. Layer 2 calls Nx2VFS's fileHandler
  3. fileHandler locates data inside game.nx2
  4. fileHandler decompresses and returns data
  5. Layer 2 passes data back to game
```

## Hook Endpoints

Both layers work by hooking low-level `ntdll.dll` APIs on Windows.

!!! question "Why ntdll.dll specifically?"

    `ntdll.dll` is the lowest-level user-mode library on Windows, sitting directly above the kernel. All higher-level file I/O APIs funnel through these ntdll functions:
    
    - Win32 APIs (`CreateFileW`, `ReadFile`) → `kernel32.dll` → `ntdll.dll`
    - C Runtime (`fopen`, `fread`) → `ucrtbase.dll` → `kernel32.dll` → `ntdll.dll`  
    - C++ STL (`std::ifstream`) → CRT → `kernel32.dll` → `ntdll.dll`
    
    By hooking at the ntdll level, we intercept **all** file operations from any software on Windows. This works because Windows does not have stable syscalls; all user-mode software must use OS provided APIs, which pass through `ntdll.dll` to communicate with the kernel counterparts, e.g. `NtOpenFile` -> `ZwOpenFile`.
    
    This single interception point also works with Wine on Linux, since `Wine` aims to implement Win32 as closely as possible; and that includes its relationship between `kernel32.dll` and `ntdll.dll`.

!!! note "This graph was last updated in 6th December 2025."

    Using Windows 11 25H2 as reference.
    
    Irrelevant APIs (e.g. Path Conversion `RtlDosPathNameToRelativeNtPathName`) are omitted for clarity; these converted paths will be passed to our hooks, e.g. `NtCreateFile_Hook`, in which case we do not need to concern ourselves with them.

    This graph focuses on the ***entry points*** into the VFS. Some redundant calls are considered. e.g. A function calling `NtClose` after calling `NtCreateFile` to clean up will point only to `NtCreateFile`. (We don't do anything in `NtClose`, other than update internal state.)

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]

    %% Definitions
    FindFirstFileA
    FindFirstFileExA
    FindFirstFileW
    FindFirstFileExW
    FindFirstFileExFromAppW
    FindNextFileA
    FindNextFileW

    CreateDirectory2A
    CreateDirectory2W
    CreateDirectoryA
    CreateDirectoryW
    InternalCreateDirectoryW
    InternalCreateDirectoryW_Old
    CreateFileA
    CreateFileW
    CreateFile2
    CreateFile3
    CreateFileInternal
    CreateFile2FromAppW
    CreateFileFromAppW
    CreateFileTransactedA
    CreateFileTransactedW
    CreateDirectoryExW
    CreateDirectoryFromAppW
    CreateDirectoryTransactedA
    CreateDirectoryTransactedW
    DeleteFile2A
    DeleteFile2W
    DeleteFileA
    DeleteFileW
    DeleteFileFromAppW
    InternalDeleteFileW
    GetCompressedFileSizeA
    GetCompressedFileSizeW
    CloseHandle

    CreateFileMapping2
    CreateFileMappingFromApp
    CreateFileMappingNumaA
    CreateFileMappingNumaW
    CreateFileMappingW

    CreateHardLinkA
    CreateHardLinkW

    CopyFileA
    CopyFileW
    CopyFile2
    CopyFileExA
    CopyFileExW
    CopyFileFromAppW
    CopyFileTransactedA
    CopyFileTransactedW
    BasepCopyFileExW

    GetFileAttributesA
    GetFileAttributesExA
    GetFileAttributesExFromAppW
    GetFileAttributesExW
    GetFileAttributesW
    SetFileAttributesA
    SetFileAttributesFromAppW
    SetFileAttributesW

    RemoveDirectoryA
    RemoveDirectoryFromAppW
    RemoveDirectoryW

    %%% Win32 Internal Redirects
    FindFirstFileA --> FindFirstFileExW
    FindFirstFileExA --> FindFirstFileExW
    FindFirstFileExFromAppW --> FindFirstFileExW
    FindNextFileA --> FindNextFileW
    CreateDirectory2A --> InternalCreateDirectoryW
    CreateDirectory2W --> InternalCreateDirectoryW
    CreateDirectoryA --> CreateDirectoryW
    CreateDirectoryW --> InternalCreateDirectoryW
    CreateDirectoryW --> InternalCreateDirectoryW_Old
    CreateDirectoryTransactedA --> CreateDirectoryTransactedW
    CreateDirectoryTransactedW --> CreateDirectoryW
    CreateDirectoryTransactedW --> CreateDirectoryExW
    CreateFileA --> CreateFileInternal
    CreateFileW --> CreateFileInternal
    CreateFile2 --> CreateFileInternal
    CreateFile3 --> CreateFileInternal
    CreateFile2FromAppW --> CreateFile2
    CreateFileTransactedA --> CreateFileTransactedW
    CreateFileTransactedW --> CreateFileW
    CreateDirectoryFromAppW --> CreateDirectoryW
    CreateFileFromAppW --> CreateFile2FromAppW
    DeleteFile2A --> InternalDeleteFileW
    DeleteFile2W --> InternalDeleteFileW
    DeleteFileFromAppW --> DeleteFileW
    DeleteFileA --> DeleteFileW
    DeleteFileW --> InternalDeleteFileW
    GetCompressedFileSizeA --> GetCompressedFileSizeW
    GetFileAttributesA --> GetFileAttributesW
    GetFileAttributesExA --> GetFileAttributesExW
    GetFileAttributesExFromAppW --> GetFileAttributesExW
    RemoveDirectoryA --> RemoveDirectoryW
    RemoveDirectoryFromAppW --> RemoveDirectoryW
    SetFileAttributesFromAppW --> SetFileAttributesW
    SetFileAttributesA --> SetFileAttributesW
    CreateFileMappingNumaA --> CreateFileMappingNumaW
    CreateFileMappingFromApp --> CreateFileMappingNumaW
    CreateHardLinkA --> CreateHardLinkW
    CopyFileA --> CopyFileW
    CopyFileW --> CopyFileExW
    CopyFileExA --> CopyFileExW
    CopyFileExW --> BasepCopyFileExW
    CopyFile2 --> BasepCopyFileExW
    CopyFileFromAppW --> CopyFileW
    CopyFileTransactedA --> CopyFileExA
    CopyFileTransactedW --> CopyFileExW
    end

    subgraph NT API
    %% Definitions
    NtCreateFile
    NtOpenFile
    NtQueryDirectoryFile
    NtQueryDirectoryFileEx
    NtDeleteFile
    NtQueryAttributesFile
    NtQueryFullAttributesFile
    NtQueryInformationFile
    NtSetInformationFile
    NtCreateSection
    NtCreateSectionEx
    NtClose

    %%% Win32 -> NT API
    FindFirstFileExW --> NtOpenFile
    FindFirstFileExW --> NtQueryDirectoryFileEx
    FindFirstFileW --> NtOpenFile
    FindFirstFileW --> NtQueryDirectoryFileEx
    FindNextFileW --> NtQueryDirectoryFileEx
    CreateFileInternal --> NtCreateFile
    CreateFileInternal --> NtSetInformationFile
    CreateFileInternal --> NtQueryInformationFile
    CreateDirectoryW --> NtCreateFile
    InternalCreateDirectoryW --> NtCreateFile
    InternalCreateDirectoryW_Old --> NtCreateFile
    CreateDirectoryExW --> NtOpenFile
    CreateDirectoryExW --> NtQueryInformationFile
    CreateDirectoryExW --> NtCreateFile
    CreateDirectoryExW --> NtSetInformationFile
    InternalDeleteFileW --> NtOpenFile
    InternalDeleteFileW --> NtQueryInformationFile
    InternalDeleteFileW --> NtSetInformationFile
    RemoveDirectoryW --> NtOpenFile
    GetCompressedFileSizeW --> NtOpenFile
    CloseHandle --> NtClose
    CreateFileMapping2 --> NtCreateSectionEx
    CreateFileMappingNumaW --> NtCreateSection
    CreateFileMappingW --> NtCreateSection
    CreateHardLinkW --> NtOpenFile
    CreateHardLinkW --> NtSetInformationFile
    BasepCopyFileExW --> NtCreateFile
    BasepCopyFileExW --> NtQueryInformationFile
    BasepCopyFileExW --> NtSetInformationFile
    GetFileAttributesExW --> NtQueryFullAttributesFile
    GetFileAttributesW --> NtQueryAttributesFile
    SetFileAttributesW --> NtOpenFile
    end

    %%% Hooks
    subgraph Hooks
    NtCreateFile_Hook
    NtOpenFile_Hook
    NtQueryDirectoryFileEx_Hook
    NtDeleteFile_Hook
    NtQueryAttributesFile_Hook
    NtQueryFullAttributesFile_Hook
    NtClose_Hook

    %% NT API -> Hooks
    NtCreateFile --> NtCreateFile_Hook
    NtOpenFile --> NtOpenFile_Hook
    NtQueryDirectoryFileEx --> NtQueryDirectoryFileEx_Hook
    NtQueryDirectoryFile --> NtQueryDirectoryFile_Hook

    NtDeleteFile --> NtDeleteFile_Hook
    NtQueryAttributesFile --> NtQueryAttributesFile_Hook
    NtQueryFullAttributesFile --> NtQueryFullAttributesFile_Hook
    NtClose --> NtClose_Hook
    end
```

??? note "Notable Functions we probably won't ever need but FYI"

    - `NtFsControlFile` - Making sparse files, enabling NTFS compression, create junctions.
    - `NtQueryEaFile` - Extended Attributes. DOS attributes, NTFS security descriptors, etc. Games can't have these, Windows specific and stores don't support it. Only kernel side `ZwQueryEaFile` is publicly documented by MSFT.
    - ✅ `BasepCopyFileExW` - (omitted a few sub-functions due to duplicated Ntdll call target)

??? note "Roots (as of Windows 11 25H2)"

    [Fileapi.h](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectory2a) and [Winbase.h](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createdirectorytransacteda) is also a good resource.

    KernelBase.dll (Files):

    - ✅ `CreateDirectory2A`
    - ✅ `CreateDirectory2W`
    - ✅ `InternalCreateDirectoryW`
    - ✅ `CreateDirectoryA`
    - ✅ `CreateDirectoryExW`
    - ✅ `CreateDirectoryFromAppW`
    - ✅ `CreateFile2`
    - ✅ `CreateFile2FromAppW`
    - ✅ `CreateFile3`
    - ✅ `CreateFileA`
    - ✅ `CreateFileFromAppW`
    - ✅ `CreateFileW`

    KernelBase.dll (No-op):

    - `FindClose` -> `NtClose`

    KernelBase.dll (Memory Mapping):

    - ✅ `CreateFileMapping2`
    - ✅ `CreateFileMappingFromApp`
    - ✅ `CreateFileMappingNumaA`
    - ✅ `CreateFileMappingNumaW`
    - ✅ `CreateFileMappingW`
  
    KernelBase.dll (Copy):

    - ✅ `CopyFileA`
    - ✅ `CopyFileW`
    - ✅ `CopyFile2`
    - ✅ `CopyFileExA`
    - ✅ `CopyFileExW`
    - ✅ `CopyFileFromAppW`

    KernelBase.dll (Links / Write):

    - ✅ `CreateHardLinkA`
    - ✅ `CreateHardLinkW`
    - ✅ `DeleteFile2A`
    - ✅ `DeleteFile2W`
    - ✅ `DeleteFileA`
    - ✅ `DeleteFileW`
    - ✅ `DeleteFileFromAppW`
    - ✅ `InternalDeleteFileW`
  
    Transactional NTFS (Deprecated):

    - This is a feature introduced in Windows Vista (2007), and deprecated in Windows 8 in 2012. It was deprecated due to lack of adoption and complexity.
    - Documentation heavily discourages its use and notes it as 'slated for removal'.
    - It in fact wasn't even moved from `kernel32.dll` to `kernelbase.dll`.
    - I've never to date seen a program that uses this feature.
    - Behind the scenes this uses the regular APIs, but wrapped around `RtlGetCurrentTransaction` and `RtlSetCurrentTransaction` calls. 
    - ✅ `CopyFileTransactedA`
    - ✅ `CopyFileTransactedW`
    - ✅ `CreateDirectoryTransactedA`
    - ✅ `CreateDirectoryTransactedW`
    - ✅ `CreateFileTransactedA`
    - ✅ `CreateFileTransactedW`

    KernelBase.dll (UWP - uses another process - not investigated):

    - BrokeredCreateDirectoryW

    KernelBase.dll (WTF?):

    - `CreateFileDowngrade_Win7` - 1 liner that adds a flag to a pointer passed in.

!!! note "On Windows 10 1709+, `NtQueryDirectoryFileEx` API becomes available and `NtQueryDirectoryFile` acts as a wrapper around it."

    In the VFS we would hook both, and detect if one recurses to the other using a semaphore. If we're recursing from `NtQueryDirectoryFile` to `NtQueryDirectoryFileEx`, we skip the hook code.

!!! info "This currently only contains information for Windows."

    Native support for other OSes will be added in the future.

!!! warning "TODO: GetFinalPathNameByHandleW"

    I didn't add this function to the flowchart yet.

    But basically it's Kernel32 GetFinalPathNameByHandleW -> NTDLL NtQueryObject and NtQueryInformationFile

    I wrote some [more details here](https://github.com/ModOrganizer2/modorganizer/issues/2039#issuecomment-2151221938)

### Layer 1: Virtual FileSystem

!!! info "Reminder: [Layer 1 deals with the 'where' problem](#layer-1-virtual-filesystem)"

- **`NtCreateFile`** & **`NtOpenFile`**
    - Intercept file creation/open operations. 
      - Check if path should be redirected when creating new files.
      - Substitute with target path before calling original API.
    - For 'virtual files', spoof creation to succeed without touching disk.

- **`NtQueryDirectoryFile`** & **`NtQueryDirectoryFileEx`**
    - Inject virtual files into directory search results. 
      - When application searches a directory, inject registered virtual files into the result set.
    - Uses semaphore to avoid recursion between the two APIs on Windows 10+.

- **`NtQueryAttributesFile`** && **`NtQueryFullAttributesFile`** - Return metadata for virtual/redirected files.

- **`NtClose`** - Track when file handles are closed. Used for internal handle lifecycle management.

If/when we implement write support, we would also hook APIs such as:

- **`NtDeleteFile`** - Handle deletion operations on virtual/redirected files. Intercept deletion requests and handle appropriately.

## Layer 2: File Emulation Framework

### What It Does

Synthesizes file data on-the-fly by intercepting read operations. Instead of returning data from disk, Layer 2 generates the file content dynamically by merging data from multiple sources.

Uses Layer 1 to make emulated files visible in directory searches and to handle path routing.

### Hooked APIs

- **`NtCreateFile` & `NtOpenFile`** - Detect when an emulated file is being opened. Match the file path against registered emulator routes. If matched, call into the emulator's `try_create_file` method to initialize emulator state and create internal data structures for synthesizing the file.

- **`NtReadFile`** - Intercept file read operations. If the file is being emulated, use the StreamSlice array to determine where data comes from. Read from source files/locations and return synthesized data instead of the original file data.

- **`NtSetInformationFile`** - Intercept handle update operations. Track file pointer position updates (seek operations). Emulated files need to maintain their own file pointer state so that read operations know where to read from.

- **`NtQueryInformationFile`** - Intercept file information queries. Report the emulated file's size and attributes. The emulated file size may differ from the original file on disk.

- **`NtQueryFullAttributesFile`** - Intercept file attribute queries. Report the emulated file's size and full attributes. Used when applications check file metadata without opening the file.

- **`NtClose`** - Intercept file close operations. Dispose of emulator internal state for the emulated file (such as current read offset). Free internal data structures for that emulated file instance.

**[→ Complete Hook Details](File-Emulation-Framework/Implementation-Details/Hooks.md)**

### Dependencies on Layer 1

- Calls `RegisterVirtualFile()` to make emulated files visible in directory listings
- Leverages Layer 1's redirect system for route-based file targeting
- Layer 1 handles the path redirection; Layer 2 handles the data synthesis

### Data Structures

**Emulated File Tracking:**

- Hash table keyed by **file handle** (`HANDLE` on Windows)
- Value contains emulator instance state for that specific file

**StreamSlice Array:**

Each emulated file contains an array of `StreamSlice` objects representing where data comes from:

```rust
struct StreamSlice {
    offset: u64,      // Where in emulated file this data appears
    length: u64,      // How much data this slice provides
    source: Source,   // Where to read data from
}

enum Source {
    File { handle: HANDLE, offset: u64 },  // Read from another file
    Memory { ptr: *const u8 },              // Read from memory
    Zeros,                                   // Return zeros (padding)
}
```

For each read operation:

1. Binary search the StreamSlice array to find which slice(s) cover the requested range
2. For each slice, read from the appropriate source
3. Combine the data and return to application

**Performance Characteristics:**

- Hash table lookup: ~8ns (constant time)
- Binary search on StreamSlice array: 2.5-5.5ns for <64 slices, 35ns for 16384 slices (logarithmic)
- Total overhead per read: ~15ns for typical files

### Implementation Notes

- **Zero-copy when possible:** If reading entire file from single source, can pass through directly
- **Lazy initialization:** Emulator state created only when file is actually opened
- **Route matching:** Uses suffix matching on normalized paths
- **Priority system:** More specific routes take precedence over generic ones
- **Thread safety:** Each file handle has independent state; thread-safe for concurrent operations on different handles

**[→ Full Implementation Details](File-Emulation-Framework/About.md)**

---

## How does this compare with my previous work?

!!! info "Production-Tested Foundation"

    The architecture described here builds upon the one in Reloaded-II; which have been used in production for a few years.

This documentation iterates and improves upon two major implementations:

### FileEmulationFramework (Reloaded-II)

**[FileEmulationFramework](https://github.com/Sewer56/FileEmulationFramework)** is the reference implementation for the Layer 2 architecture (with built in Layer 3 plugin). Actively used in production since 2022, across several archive formats and games. An iteration of another project of mine from 2020.

### reloaded.universal.redirector

**reloaded.universal.redirector** implements the Layer 1 architecture with comprehensive read operation support in the unreleased `rewrite-usvfs-read-features` branch (1). Original simpler file open hook actively used in production since 2019.
{ .annotate }

1.  Fully working and complete, including on Wine, but unreleased due to certainty it would break with future .NET Runtime upgrades from problems involving GC transitions.<br/><br/>A certain case in Wine already reproduced the inevitable.

### Key Architectural Difference

**Separation of Archive Emulation and Virtual Files**

Reloaded-II's `FileEmulationFramework` was originally designed for **archive replacement** - creating emulated archives lazily when accessed to avoid generating massive files up front.

!!! example "Original Use Case"
    Replace a 60GB game archive with an emulated version, without using disk space.

Over time, new requirements emerged: **NxVFS** integration and **synthesizing arbitrary files** (e.g., adding files to a game's load list). This created architectural inconsistency - some files lazy, others up front - and unnecessary coupling between archive emulation and virtual file management.

**The new solution**: Split these as separate concerns. The VFS generates all emulated files **up front** during registration, while archive emulation is handled distinctly when needed. This provides predictable state, reduced overhead at file open time, and cleaner architecture. The marginal memory savings from lazy initialization weren't worth the complexity.

### Other Differences

- Added support for Memory Maps.
- Added missing NtOpenFile API. (& other related miscellany)
- APIs use handles to unregister/dispose.