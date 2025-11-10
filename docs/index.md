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

## Filesystem Architecture

!!! info "Understanding OS filesystem architecture"

    This section documents how filesystem APIs are structured on each platform.
    
    Understanding these architectures is essential for implementing the VFS.

### Windows

!!! tip "All user-mode file I/O APIs funnel through `ntdll.dll`"

    `ntdll.dll` is the lowest-level user-mode library on Windows. It is a wrapper around kernel system calls to `ntoskrnl.exe`.
    
    All higher-level file I/O APIs eventually call `ntdll.dll` functions:
    
    - Win32 APIs (`CreateFileW`, `ReadFile`) → `kernel32.dll` → `ntdll.dll` → `ntoskrnl.exe` (via unstable syscalls)
    - C Runtime (`fopen`, `fread`) → `ucrtbase.dll` → `kernel32.dll` → `ntdll.dll` → `ntoskrnl.exe` (via unstable syscalls)
    - C++ STL (`std::ifstream`) → CRT → `kernel32.dll` → `ntdll.dll` → `ntoskrnl.exe` (via unstable syscalls)
    
    This makes `ntdll.dll` the single interception point for all file operations on Windows.
    
    **Critical:** Unlike Linux, Windows syscall numbers are **not stable** between versions. This is why `ntdll.dll` exists as a stable abstraction layer- all normal user-mode software goes through it.

!!! note "These graphs were last updated in 6th-9th December 2025."

    Using Windows 11 25H2 as reference.
    
    Irrelevant APIs (e.g. Path Conversion `RtlDosPathNameToRelativeNtPathName`) are omitted for clarity; these converted paths will be passed to our hooks, e.g. `NtCreateFile_Hook`, in which case we do not need to concern ourselves with them.

    This graph focuses on the ***entry points*** into `ntdll.dll` (and thus, the VFS). Redundant calls are omitted for clarity. For example, when a function calls `NtCreateFile` and then `NtClose` to clean up the file handle, only the `NtCreateFile` call is shown in the graph.
    
    (We don't do anything in `NtClose`, other than update internal state.)

!!! tip "Chart Organization"

    The API flow charts are split into logical groups based on functionality and dependencies. Each chart shows how Win32 APIs funnel down to NT API entry points.

#### Directory Enumeration

All `FindFirst*` and `FindNext*` APIs converge through internal functions to `NtQueryDirectoryFileEx` for directory listing operations.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    FindFirstFileA
    FindFirstFileExA
    FindFirstFileW
    FindFirstFileExW
    FindFirstFileExFromAppW
    FindFirstFileTransactedA
    FindFirstFileTransactedW
    InternalFindFirstFileExW
    InternalFindFirstFileW
    FindNextFileA
    FindNextFileW

    FindFirstFileA --> InternalFindFirstFileExW
    FindFirstFileExA --> InternalFindFirstFileW
    FindFirstFileExFromAppW --> FindFirstFileExW
    FindFirstFileExW --> InternalFindFirstFileExW
    FindFirstFileW --> InternalFindFirstFileW
    FindFirstFileTransactedA --> FindFirstFileExA
    FindFirstFileTransactedW --> FindFirstFileExW
    FindNextFileA --> FindNextFileW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtOpenFile
    NtQueryDirectoryFileEx

    InternalFindFirstFileExW --> NtOpenFile
    InternalFindFirstFileExW --> NtQueryDirectoryFileEx
    InternalFindFirstFileW --> NtOpenFile
    InternalFindFirstFileW --> NtQueryDirectoryFileEx
    FindNextFileW --> NtQueryDirectoryFileEx
    end
```

#### File & Directory Creation

All `CreateFile*` and `CreateDirectory*` APIs funnel through internal functions to `NtCreateFile`, along with optional metadata operations.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
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
    OpenFileById
    ReOpenFile

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
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtCreateFile
    NtSetInformationFile
    NtQueryInformationFile
    NtOpenFile

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
    OpenFileById --> NtCreateFile
    ReOpenFile --> NtCreateFile
    end
```

!!! info "ReOpenFile"

    Operates on existing handle (already redirected). No path redirection needed.

#### File & Directory Deletion

All deletion APIs (`DeleteFile*` and `RemoveDirectory*`) converge through internal functions (`InternalDeleteFileW` and `InternalRemoveDirectoryW`) to NT-level operations.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    DeleteFile2A
    DeleteFile2W
    DeleteFileA
    DeleteFileW
    DeleteFileFromAppW
    InternalDeleteFileW
    RemoveDirectory2A
    RemoveDirectory2W
    RemoveDirectoryA
    RemoveDirectoryFromAppW
    RemoveDirectoryW
    RemoveDirectoryTransactedA
    RemoveDirectoryTransactedW
    InternalRemoveDirectoryW

    DeleteFile2A --> InternalDeleteFileW
    DeleteFile2W --> InternalDeleteFileW
    DeleteFileFromAppW --> DeleteFileW
    DeleteFileA --> DeleteFileW
    DeleteFileW --> InternalDeleteFileW
    RemoveDirectory2A --> InternalRemoveDirectoryW
    RemoveDirectory2W --> InternalRemoveDirectoryW
    RemoveDirectoryA --> RemoveDirectoryW
    RemoveDirectoryFromAppW --> RemoveDirectoryW
    RemoveDirectoryTransactedA --> RemoveDirectoryTransactedW
    RemoveDirectoryTransactedW --> RemoveDirectoryW
    RemoveDirectoryW --> InternalRemoveDirectoryW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtOpenFile
    NtQueryInformationFile
    NtSetInformationFile

    InternalDeleteFileW --> NtOpenFile
    InternalDeleteFileW --> NtQueryInformationFile
    InternalDeleteFileW --> NtSetInformationFile
    InternalRemoveDirectoryW --> NtOpenFile
    InternalRemoveDirectoryW --> NtQueryInformationFile
    InternalRemoveDirectoryW --> NtSetInformationFile
    end
```

#### Read/Write Operations

All file read and write operations, including file pointer positioning and file size modification, funnel through NT-level read/write APIs.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    ReadFile
    ReadFileEx
    ReadFileScatter
    WriteFile
    WriteFileEx
    WriteFileGather
    SetFilePointer
    SetFilePointerEx
    SetEndOfFile

    end

    subgraph NT["NT API (ntdll.dll)"]
    NtReadFile
    NtReadFileScatter
    NtWriteFile
    NtWriteFileGather
    NtQueryInformationFile
    NtSetInformationFile

    ReadFile --> NtReadFile
    ReadFileEx --> NtReadFile
    ReadFileScatter --> NtReadFileScatter
    WriteFile --> NtWriteFile
    WriteFileEx --> NtWriteFile
    WriteFileGather --> NtWriteFileGather
    SetFilePointer --> NtQueryInformationFile
    SetFilePointer --> NtSetInformationFile
    SetFilePointerEx --> NtQueryInformationFile
    SetFilePointerEx --> NtSetInformationFile
    SetEndOfFile --> NtQueryInformationFile
    SetEndOfFile --> NtSetInformationFile
    end
```

#### File Attributes

Query and modification of file attributes. Path-based queries use `GetFileAttributes*` and `SetFileAttributes*` APIs, handle-based queries use `GetFileInformationByHandle*` APIs, and name-based queries use `GetFileInformationByName`.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    GetFileAttributesA
    GetFileAttributesExA
    GetFileAttributesExFromAppW
    GetFileAttributesExW
    GetFileAttributesW
    SetFileAttributesA
    SetFileAttributesFromAppA
    SetFileAttributesFromAppW
    SetFileAttributesW
    InternalSetFileAttributesW
    GetFileInformationByHandle
    GetFileInformationByHandleEx
    SetFileInformationByHandle
    GetFileInformationByName
    GetFileSize
    GetFileSizeEx
    GetFileTime
    SetFileTime
    GetFileType
    GetCompressedFileSizeA
    GetCompressedFileSizeW
    GetFinalPathNameByHandleA
    GetFinalPathNameByHandleW

    GetFileAttributesA --> GetFileAttributesW
    GetFileAttributesExA --> GetFileAttributesExW
    GetFileAttributesExFromAppW --> GetFileAttributesExW
    SetFileAttributesA --> SetFileAttributesW
    SetFileAttributesFromAppA --> SetFileAttributesW
    SetFileAttributesFromAppW --> SetFileAttributesW
    SetFileAttributesW --> InternalSetFileAttributesW
    GetCompressedFileSizeA --> GetCompressedFileSizeW
    GetFinalPathNameByHandleA --> GetFinalPathNameByHandleW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtQueryAttributesFile
    NtQueryFullAttributesFile
    NtQueryInformationFile
    NtQueryInformationByName
    NtQueryVolumeInformationFile
    NtQueryDirectoryFile
    NtQueryObject
    NtOpenFile
    NtSetInformationFile

    GetFileAttributesExW --> NtQueryFullAttributesFile
    GetFileAttributesW --> NtQueryAttributesFile
    InternalSetFileAttributesW --> NtOpenFile
    InternalSetFileAttributesW --> NtSetInformationFile
    GetFileInformationByHandle --> NtQueryVolumeInformationFile
    GetFileInformationByHandle --> NtQueryInformationFile
    GetFileInformationByHandleEx --> NtQueryDirectoryFile
    GetFileInformationByHandleEx --> NtQueryInformationFile
    GetFileInformationByHandleEx --> NtQueryVolumeInformationFile
    GetFileInformationByName --> NtQueryInformationByName
    GetFileSize --> NtQueryInformationFile
    GetFileSizeEx --> NtQueryInformationFile
    GetFileTime --> NtQueryInformationFile
    GetFileType --> NtQueryVolumeInformationFile
    GetCompressedFileSizeW --> NtOpenFile
    GetCompressedFileSizeW --> NtQueryInformationFile
    GetFinalPathNameByHandleW --> NtQueryObject
    GetFinalPathNameByHandleW --> NtQueryInformationFile
    SetFileInformationByHandle --> NtSetInformationFile
    SetFileTime --> NtSetInformationFile
    end
```

!!! info "NtQueryVolumeInformationFile does not need emulation"

    `NtQueryVolumeInformationFile` queries volume-level information (filesystem type, serial number, etc.) rather than individual file metadata. Since we're not virtualizing entire volumes, this API can pass through without interception.

!!! info "GetCompressedFileSize* APIs"

    `GetCompressedFileSizeA` and `GetCompressedFileSizeW` query the on-disk size of NTFS compressed files (which differs from logical file size for compressed files). For virtual files, return the regular file size. For redirected files, simply redirect the path and let the underlying file system report its compressed size.

!!! info "GetFileVersion* APIs"

    `GetFileVersionInfoA`, `GetFileVersionInfoW`, `GetFileVersionInfoExA`, `GetFileVersionInfoExW`, and related APIs extract embedded version resources from PE files. These are handled by the standard file read/open APIs (`NtCreateFile`, `NtReadFile`) and don't require separate hooking.

#### File Copy, Move & Replace Operations

All `CopyFile*` variants converge through `BasepCopyFileExW`. `MoveFile*` variants converge through `MoveFileWithProgressTransactedW` or `MoveFileWithProgressW`, with some move operations delegating to copy for cross-volume moves. `ReplaceFile*` variants converge through `ReplaceFileExInternal`.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    CopyFileA
    CopyFileW
    CopyFile2
    CopyFileExA
    CopyFileExW
    CopyFileFromAppW
    CopyFileTransactedA
    CopyFileTransactedW
    MoveFileA
    MoveFileW
    MoveFileExA
    MoveFileExW
    MoveFileWithProgressA
    MoveFileWithProgressW
    MoveFileTransactedA
    MoveFileTransactedW
    MoveFileWithProgressTransactedA
    MoveFileWithProgressTransactedW
    MoveFileFromAppW
    ReplaceFileA
    ReplaceFileW
    ReplaceFileFromAppW
    ReplaceFileExInternal
    BasepCopyFileExW

    CopyFileA --> CopyFileW
    CopyFileW --> CopyFileExW
    CopyFileExA --> CopyFileExW
    CopyFileExW --> BasepCopyFileExW
    CopyFile2 --> BasepCopyFileExW
    CopyFileFromAppW --> CopyFileW
    CopyFileTransactedA --> CopyFileExA
    CopyFileTransactedW --> CopyFileExW
    MoveFileA --> MoveFileWithProgressTransactedA
    MoveFileWithProgressTransactedA --> MoveFileWithProgressTransactedW
    MoveFileExA --> MoveFileWithProgressTransactedA
    MoveFileTransactedA --> MoveFileWithProgressTransactedA
    MoveFileTransactedW --> MoveFileWithProgressTransactedW
    MoveFileW --> MoveFileWithProgressW
    MoveFileWithProgressA --> MoveFileWithProgressTransactedA
    MoveFileExW --> MoveFileWithProgressTransactedW
    MoveFileFromAppW --> MoveFileWithProgressW
    MoveFileWithProgressW --> MoveFileWithProgressTransactedW
    MoveFileWithProgressTransactedW --> BasepCopyFileExW
    ReplaceFileA --> ReplaceFileW
    ReplaceFileW --> ReplaceFileExInternal
    ReplaceFileFromAppW --> ReplaceFileW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtCreateFile
    NtOpenFile
    NtQueryInformationFile
    NtSetInformationFile
    NtFsControlFile
    NtQueryVolumeInformationFile

    BasepCopyFileExW --> NtCreateFile
    BasepCopyFileExW --> NtQueryInformationFile
    BasepCopyFileExW --> NtSetInformationFile
    MoveFileWithProgressTransactedW --> NtOpenFile
    MoveFileWithProgressTransactedW --> NtQueryInformationFile
    MoveFileWithProgressTransactedW --> NtSetInformationFile
    ReplaceFileExInternal --> NtOpenFile
    ReplaceFileExInternal --> NtFsControlFile
    ReplaceFileExInternal --> NtQueryInformationFile
    ReplaceFileExInternal --> NtSetInformationFile
    ReplaceFileExInternal --> NtQueryVolumeInformationFile
    end
```

#### Links & Symbolic Links

Creation and enumeration of hard links and symbolic links through dedicated APIs.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    CreateHardLinkA
    CreateHardLinkW
    CreateSymbolicLinkA
    CreateSymbolicLinkW
    CreateSymbolicLinkTransactedA
    CreateSymbolicLinkTransactedW
    FindFirstFileNameW
    FindNextFileNameW
    FindParent

    CreateHardLinkA --> CreateHardLinkW
    CreateSymbolicLinkA --> CreateSymbolicLinkW
    CreateSymbolicLinkTransactedA --> CreateSymbolicLinkTransactedW
    CreateSymbolicLinkTransactedW --> CreateSymbolicLinkW
    FindNextFileNameW --> FindParent
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtOpenFile
    NtCreateFile
    NtSetInformationFile
    NtQueryInformationFile

    CreateHardLinkW --> NtOpenFile
    CreateHardLinkW --> NtSetInformationFile
    CreateSymbolicLinkW --> NtCreateFile
    CreateSymbolicLinkW --> NtSetInformationFile
    FindFirstFileNameW --> NtCreateFile
    FindFirstFileNameW --> NtQueryInformationFile
    FindParent --> NtCreateFile
    end
```

#### Memory Mapped Files

All `CreateFileMapping*` APIs for memory-mapped file creation converge to NT section APIs.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    CreateFileMapping2
    CreateFileMappingFromApp
    CreateFileMappingNumaA
    CreateFileMappingNumaW
    CreateFileMappingW

    CreateFileMappingNumaA --> CreateFileMappingNumaW
    CreateFileMappingFromApp --> CreateFileMappingNumaW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtCreateSection
    NtCreateSectionEx

    CreateFileMapping2 --> NtCreateSectionEx
    CreateFileMappingNumaW --> NtCreateSection
    CreateFileMappingW --> NtCreateSection
    end
```

#### Change Notifications

Directory change monitoring APIs for tracking file system modifications.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    FindFirstChangeNotificationA
    FindFirstChangeNotificationW
    FindNextChangeNotification
    ReadDirectoryChangesW
    ReadDirectoryChangesExW

    FindFirstChangeNotificationA --> FindFirstChangeNotificationW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtOpenFile
    NtNotifyChangeDirectoryFile
    NtNotifyChangeDirectoryFileEx

    FindFirstChangeNotificationW --> NtOpenFile
    FindFirstChangeNotificationW --> NtNotifyChangeDirectoryFile
    FindNextChangeNotification --> NtNotifyChangeDirectoryFile
    NtNotifyChangeDirectoryFile --> NtNotifyChangeDirectoryFileEx
    ReadDirectoryChangesW --> NtNotifyChangeDirectoryFileEx
    ReadDirectoryChangesExW --> NtNotifyChangeDirectoryFileEx
    end
```

#### Handle Lifetime Management

Handle cleanup operations that need hooking for internal state tracking.

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    CloseHandle

    end

    subgraph NT["NT API (ntdll.dll)"]
    NtClose

    CloseHandle --> NtClose
    end
```

!!! info "Why hook NtClose?"

    We need to hook `NtClose` for lifetime management - tracking when file handles are closed to clean up internal VFS state.

#### Notable Functions (Not Relevant for Games)

!!! info "We don't care about these APIs"

    The following APIs are documented for completeness but are **not relevant** for game modding:
    
    - They have not been used in games
    - They have no reason to be used in games  
    - Game stores don't support these features

```mermaid
flowchart LR
    subgraph Win32["Win32 (Kernel32.dll/KernelBase.dll)"]
    FindFirstStreamW
    FindNextStreamW
    LockFile
    LockFileEx
    UnlockFile
    UnlockFileEx
    DecryptFileA
    DecryptFileW
    EncryptFileA
    EncryptFileW
    FileEncryptionStatusA
    FileEncryptionStatusW
    GetFileSecurityA
    GetFileSecurityW
    SetFileSecurityA
    SetFileSecurityW
    SetFileShortNameA
    SetFileShortNameW

    DecryptFileA --> DecryptFileW
    EncryptFileA --> EncryptFileW
    FileEncryptionStatusA --> FileEncryptionStatusW
    GetFileSecurityA --> GetFileSecurityW
    SetFileSecurityA --> SetFileSecurityW
    SetFileShortNameA --> SetFileShortNameW
    end

    subgraph NT["NT API (ntdll.dll)"]
    NtCreateFile
    NtQueryInformationFile
    NtSetInformationFile
    NtLockFile
    NtUnlockFile
    NtQueryEaFile
    NtFsControlFile
    NtQuerySecurityObject
    NtSetSecurityObject

    FindFirstStreamW --> NtCreateFile
    FindFirstStreamW --> NtQueryInformationFile
    LockFile --> NtLockFile
    LockFileEx --> NtLockFile
    UnlockFile --> NtUnlockFile
    UnlockFileEx --> NtUnlockFile
    DecryptFileW --> NtCreateFile
    DecryptFileW --> NtSetInformationFile
    EncryptFileW --> NtCreateFile
    EncryptFileW --> NtSetInformationFile
    FileEncryptionStatusW --> NtQueryInformationFile
    GetFileSecurityW --> NtQuerySecurityObject
    SetFileSecurityW --> NtSetSecurityObject
    SetFileShortNameW --> NtSetInformationFile
    end
```

**What are these APIs:**

- **NTFS Alternate Data Streams** (`FindFirstStreamW`, `FindNextStreamW`) - Unsupported by game stores.
- **File Locking** (`LockFile`, `LockFileEx`, `UnlockFile`, `UnlockFileEx`) - Never seen a program that uses these APIs.
- **File Encryption** (`DecryptFile*`, `EncryptFile*`, `FileEncryptionStatus*`, `OpenEncryptedFileRaw*`) - Not supported with any game store, or even legacy games.
- **Security Descriptors** (`GetFileSecurity*`, `SetFileSecurity*`, `GetSecurityInfo`, `SetSecurityInfo`, `GetNamedSecurityInfo*`, `SetNamedSecurityInfo*`) - ACL management. Not supported with any game store, or even legacy games.
- **DOS Short Names** (`SetFileShortName*`) - Legacy DOS 8.3 filename support. Not needed for games.
- **LZ Expansion** ([lzexpand.h](https://learn.microsoft.com/en-us/windows/win32/api/lzexpand/)) - Ancient deprecated API (`LZOpenFileW` → `LZOpenFileA` → `OpenFile`, etc.). Would be redirected with existing hooks.
- **Extended Attributes** (`NtQueryEaFile`) - DOS attributes, NTFS security descriptors, etc. Games can't have these, Windows specific and stores don't support it.
- **File System Control** (`NtFsControlFile`) - Making sparse files, enabling NTFS compression, create junctions. This operates on file handles from `NtCreateFile`, so should still be redirected nonetheless.

!!! note "Roots (as of Windows 11 25H2)"

    Look at [Fileapi.h](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectory2a) and [Winbase.h](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createdirectorytransacteda). These are good resources that are fairly up to date for Win32.
    For UWP, see [Fileapifromapp.h](https://learn.microsoft.com/en-us/windows/win32/api/fileapifromapp/nf-fileapifromapp-createfilefromappw).

    The above graphs were made using those as the starting point; combines with own searching through exports.

!!! info "About Transactional NTFS (Deprecated)"

    This is a feature introduced in Windows Vista (2007), and deprecated in Windows 8 in 2012. It was deprecated due to lack of adoption and complexity. Documentation heavily discourages its use and notes it as 'slated for removal'.

    The code for this function wasn't even moved from `kernel32.dll` to `kernelbase.dll`. Likewise, I've never to date seen a program that uses this feature.

    Behind the scenes this uses the regular APIs, but wrapped around `RtlGetCurrentTransaction` and `RtlSetCurrentTransaction` calls. So whatever we write will nonetheless work out the box. Included in the graphs for completeness.

    - ✅ `CopyFileTransactedA`
    - ✅ `CopyFileTransactedW`
    - ✅ `CreateDirectoryTransactedA`
    - ✅ `CreateDirectoryTransactedW`
    - ✅ `CreateFileTransactedA`
    - ✅ `CreateFileTransactedW`
    - ✅ `CreateSymbolicLinkTransactedA`
    - ✅ `CreateSymbolicLinkTransactedW`
    - ✅ `FindFirstFileTransactedA`
    - ✅ `FindFirstFileTransactedW`


!!! info "About WinRT/UWP Brokered 'FromApp' Functions (Windows 10 1803+)"

    Brokered calls are API calls that go through `RuntimeBroker.exe`, which acts as a security intermediary between UWP apps running in an AppContainer sandbox and system resources they need to access.
    
    The broker enforces capability-based security and permission checks.

    There are 2 types of APIs supported for WinRT/UWP:

    1. APIs such as `CreateFile2`. These are heavily restricted to only support `ApplicationData.LocalFolder` or `Package.InstalledLocation` directories. 
    2. APIs such as `CreateFile2FromAppW` will first run e.g. `CreateFile2`, and if that fails, it will route through the 'broker', i.e. `BrokeredCreateFile2` in `ext-ms-win-winrt-storage-win32broker-;1-1-0.dll`.
        - This would require an extra hook on a separate process.

    I have not experimented, but based on code inspection, it'll redirect, then likely fail due to `ApplicationData.LocalFolder`/`Package.InstalledLocation` limitation, and then try routing through the broker (separate process).

    !!! warning "This is inconsequential for most games."

        ***This section concerns ONLY TRUE UWP APPS***

        Most (pretty much all) games on the Xbox Store are Win32 titles which run using 'Desktop Bridge' a.k.a. 'Project Centennial'.

        These Apps declare `<rescap:Capability Name="runFullTrust" />` in `AppXManifest.xml`, meaning they have full access to the filesystem like regular Win32 apps.

        In those (basically all) cases, the VFS will run just fine, as it has been for a good handful of games with existing Reloaded-II mods.

        It may be possible you can just add `runFullTrust` to any pure UWP app to have it work; that I'm not sure. Never ran into a real UWP game.

!!! note "On Windows 10 1709+, `NtQueryDirectoryFileEx` API becomes available and `NtQueryDirectoryFile` acts as a wrapper around it."

    In the VFS we would hook both APIs, and detect if one recurses to the other using a semaphore. If we're recursing from `NtQueryDirectoryFile` to `NtQueryDirectoryFileEx`, we skip the hook code.
    
    **`NtNotifyChangeDirectoryFileEx`:** Conversely, `NtNotifyChangeDirectoryFile` wraps `NtNotifyChangeDirectoryFileEx` on modern Windows versions. I have not verified which version made this change.
    
    **Wine Compatibility:** These `Ex` variants are not implemented in Wine. The base APIs (`NtQueryDirectoryFile` and `NtNotifyChangeDirectoryFile`) work directly without wrapper behaviour.

### Linux

!!! info "Native Linux games, not Wine"

    This covers native Linux games only. Wine is covered by the Windows section above.

!!! tip "Linux has a stable syscall interface"

    Unlike Windows, Linux provides stable syscalls. This means programs can call into the kernel directly, though manually doing so is not advised.
    
    In practice, >99% of programs/games are built with `glibc`:
    
    program → `glibc` (`libc`) → kernel
    
    However, in some cases they may be built with `musl` (where the libc is statically linked with no exports), or using a language like Zig that syscalls directly by default.

!!! tip "Linux file I/O syscalls are simpler than Windows NT APIs"

    On Windows, you get a few very heavily overloaded functions with 10s of flags. On Linux you get a separate function for each operation, with few flags if any.
    
    Therefore it's easier to implement the VFS on Linux, as you don't have to work with every possible flag combination.

#### File I/O System Calls

!!! warning "This list needs review"

    I only had a quick glance and generated this list based on syscall list with LLM help, this needs an extra review.

The following syscalls handle file and directory operations on Linux (x86_64):

**File Opening & Creation:**

- `open` (2) - Open/create file
- `openat` (257) - Open/create file relative to directory fd
- `openat2` (437) - Extended open with more options
- `creat` (85) - Create or truncate file
- `close` (3) - Close file descriptor

**File Reading & Writing:**

- `read` (0) - Read from file
- `write` (1) - Write to file
- `pread64` (17) - Read from file at offset
- `pwrite64` (18) - Write to file at offset
- `readv` (19) - Read into multiple buffers
- `writev` (20) - Write from multiple buffers
- `preadv` (295) - Read into multiple buffers at offset
- `pwritev` (296) - Write from multiple buffers at offset
- `preadv2` (327) - Extended preadv with flags
- `pwritev2` (328) - Extended pwritev with flags

**File Position & Attributes:**

- `lseek` (8) - Reposition file offset
- `truncate` (76) - Truncate file to specified length
- `ftruncate` (77) - Truncate file using fd
- `fallocate` (285) - Preallocate space for file
- `sendfile64` (40) - Transfer data between fds
- `copy_file_range` (326) - Copy range of data between files
- `splice` (275) - Move data between pipes and files

**File Metadata & Status:**

- `stat` / `newstat` (4) - Get file status
- `fstat` / `newfstat` (5) - Get file status by fd
- `lstat` / `newlstat` (6) - Get file status (don't follow symlinks)
- `newfstatat` (262) - Get file status relative to directory fd
- `statx` (332) - Extended file status
- `access` (21) - Check file accessibility
- `faccessat` (269) - Check file accessibility relative to directory fd
- `faccessat2` (439) - Extended faccessat with flags

**File Permissions & Ownership:**

!!! info "Not needed for VFS"

    Assume user has access to both original and modded game files. No additional hooking required.

- `chmod` (90) - Change file permissions
- `fchmod` (91) - Change file permissions by fd
- `fchmodat` (268) - Change file permissions relative to directory fd
- `fchmodat2` (452) - Extended fchmodat with flags
- `chown` (92) - Change file owner and group
- `fchown` (93) - Change file owner and group by fd
- `lchown` (94) - Change file owner and group (don't follow symlinks)
- `fchownat` (260) - Change file owner and group relative to directory fd

**Directory Operations:**

- `mkdir` (83) - Create directory
- `mkdirat` (258) - Create directory relative to directory fd
- `rmdir` (84) - Remove directory
- `getdents` (78) - Get directory entries
- `getdents64` (217) - Get directory entries (64-bit)

**File & Directory Manipulation:**

- `rename` (82) - Rename file or directory
- `renameat` (264) - Rename relative to directory fds
- `renameat2` (316) - Extended rename with flags
- `link` (86) - Create hard link
- `linkat` (265) - Create hard link relative to directory fds
- `unlink` (87) - Remove file
- `unlinkat` (263) - Remove file relative to directory fd
- `symlink` (88) - Create symbolic link
- `symlinkat` (266) - Create symbolic link relative to directory fd
- `readlink` (89) - Read value of symbolic link
- `readlinkat` (267) - Read value of symbolic link relative to directory fd
- `mknod` (133) - Create special or ordinary file
- `mknodat` (259) - Create special or ordinary file relative to directory fd

**File Descriptor Operations:**

- `dup` (32) - Duplicate file descriptor
- `dup2` (33) - Duplicate file descriptor to specific fd
- `dup3` (292) - Duplicate file descriptor with flags
- `fcntl` (72) - File control operations
- `ioctl` (16) - Device-specific I/O control

**File Synchronization:**

!!! info "Possibly not needed"

    These operate on already-opened file handles, which would already be redirected.

- `sync` (162) - Synchronize cached writes to disk
- `syncfs` (306) - Synchronize filesystem
- `fsync` (74) - Synchronize file data and metadata
- `fdatasync` (75) - Synchronize file data
- `sync_file_range` (277) - Sync file region to disk

**File Locking:**

- `flock` (73) - Apply or remove advisory lock

**Time & Timestamps:**

- `utime` (132) - Change file timestamps
- `utimensat` (280) - Change file timestamps with nanosecond precision

**Extended Attributes:**

- `setxattr` (188) - Set extended attribute
- `lsetxattr` (189) - Set extended attribute (don't follow symlinks)
- `fsetxattr` (190) - Set extended attribute by fd
- `setxattrat` (463) - Set extended attribute relative to directory fd
- `getxattr` (191) - Get extended attribute
- `lgetxattr` (192) - Get extended attribute (don't follow symlinks)
- `fgetxattr` (193) - Get extended attribute by fd
- `getxattrat` (464) - Get extended attribute relative to directory fd
- `listxattr` (194) - List extended attributes
- `llistxattr` (195) - List extended attributes (don't follow symlinks)
- `flistxattr` (196) - List extended attributes by fd
- `listxattrat` (465) - List extended attributes relative to directory fd
- `removexattr` (197) - Remove extended attribute
- `lremovexattr` (198) - Remove extended attribute (don't follow symlinks)
- `fremovexattr` (199) - Remove extended attribute by fd
- `removexattrat` (466) - Remove extended attribute relative to directory fd

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

- **`NtQueryFullAttributesFile`**
    - Intercept file attribute queries.
    - Report the virtual file's size and full attributes.
    - Used when applications check file metadata without opening the file.

- **`NtCreateSection`** & **`NtCreateSectionEx`**
    - Memory-mapped file support for virtual files.
    - When applications try to memory-map a virtual file handle, create an anonymous memory section.
    - Populate the section from the `fileHandler` and map it into the process.
    - Essential for applications using memory-mapped I/O (many games load assets this way).
    - `NtCreateSectionEx` is the modern extended variant used by `CreateFileMapping2` and `CreateFileMappingFromApp`.
    - Strategy used will depend on amount of data needed to map. Small mappings will be fully populated, huge mappings will use page fault handling.

- **`NtClose`**
    - Intercept file close operations.
    - Dispose of virtual file state (such as current read offset).
    - Free internal data structures for that virtual file instance.

**[→ Complete Hook Details](File-Emulation-Framework/Implementation-Details/Hooks.md)**

#### Write Support (Future)

When writable virtual files are implemented, additional APIs will be hooked:

- **`NtWriteFile`** & **`NtWriteFileGather`**
    - Write operations to virtual files.
    - Requires extending the `fileHandler` interface to support write callbacks.
    - `NtWriteFileGather` handles scatter-gather writes (complements `NtReadFileScatter`).

### Linux

!!! info "TODO: Document Linux syscalls for Layer 2"

    This section will document the Linux syscalls hooked by Layer 2.

### Dependencies on Layer 1

- Calls `RegisterVirtualFile()` to make virtual files visible in directory listings
- Layer 1 handles the path redirection; Layer 2 handles the data synthesis
- Layer 1 makes virtual files appear in searches; Layer 2 provides their content

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