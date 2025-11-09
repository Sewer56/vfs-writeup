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

    The actual VFS implementation on Linux will be simpler than on Windows due to the straightforward syscall interface.

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

## Layer 1 Hook Endpoints

!!! info "Reminder: [Layer 1 deals with the 'where' problem](#layer-1-virtual-filesystem)"

    This section documents the specific APIs hooked by Layer 1 for each platform.

### Windows

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

- **`NtCreateFile` & `NtOpenFile`** - Detect when a virtual file is being opened. Look up the registered `fileHandler` for this path and initialize state for managing read operations.

- **`NtReadFile`** - Intercept file read operations. If the file is virtual, delegate to the `fileHandler` to provide the actual data at the requested offset.

- **`NtSetInformationFile`** - Intercept handle update operations. Track file pointer position updates (seek operations). Virtual files need to maintain their own file pointer state.

- **`NtQueryInformationFile`** - Intercept file information queries. Report the virtual file's size and attributes from the registered metadata.

- **`NtQueryFullAttributesFile`** - Intercept file attribute queries. Report the virtual file's size and full attributes. Used when applications check file metadata without opening the file.

- **`NtClose`** - Intercept file close operations. Dispose of virtual file state (such as current read offset). Free internal data structures for that virtual file instance.

**[→ Complete Hook Details](File-Emulation-Framework/Implementation-Details/Hooks.md)**

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