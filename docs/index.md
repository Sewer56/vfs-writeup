!!! info "About This Documentation"

	Forked and improved from the my original [Reloaded3 Specification](https://reloaded-project.github.io/Reloaded-III/Mods/Essentials/Virtual-FileSystem/About.html); still carries some carryovers from there, some names included.

# User Space Virtual Filesystems

This wiki describes the implementation details of a **two-layer architecture** for implementing a Virtual FileSystem suitable for modding games.

## Table of Contents

**Overview**

- **[User Space Virtual Filesystems](index.md)** - [This Page] Two-layer VFS architecture for non-invasive game modding

**Core Architecture**

- **[Filesystem Architecture](Filesystem-Architecture.md)** - How I/O APIs relate to each other in Windows and Linux
    - Required understanding to implement VFS
- **[Behaviours & Limitations](Behaviours.md)** - Design constraints, write behaviour, platform-specific limitations, and edge cases
- **[DLL Hijacking](DLL-Hijacking.md)** - Loading VFS before external mod loaders using PE import patching
    - Needed for software like Nexus Mods App that wrap around external mod loaders

**Hooking Implementation**

- **[Overview](Hooking-Implementation/Overview.md)** - Hooking strategy and implementation approach for intercepting file I/O
- **[File Redirection (Layer 1)](Hooking-Implementation/File-Redirection.md)** - Which APIs to hook for path redirection and virtual file visibility
- **Virtual Files (Layer 2)** - Handling virtual file data synthesis and read operations
    - **[Standard I/O](Hooking-Implementation/Virtual-Files/Standard-IO.md)** - Hooking read/write operations for virtual files
    - **[Memory Mapped Files](Hooking-Implementation/Virtual-Files/Memory-Mapped-Files.md)** - Emulating memory-mapped virtual files with page fault handling
    - **[DirectStorage & IoRing](Hooking-Implementation/Virtual-Files/DirectStorage.md)** - Supporting the new Windows 11 IoRing API

**Virtual FileSystem Architecture**

- **[Layer 1: Virtual FileSystem](Virtual-FileSystem/About.md)** - Path redirection and virtual file injection without administrator rights or filesystem modifications
- **[Layer 2: Virtual File Framework](File-Emulation-Framework/About.md)** - Handles virtual file data synthesis and read operations for extensions
- **[Layer 3: Archive Emulation Framework](File-Emulation-Framework/About.md)** - Extensions for injecting files into game archives without writing code

!!! info "Layer 2 and Layer 3 documentation is incomplete"

    Provided as reference only, not final.
    
    This has been copied as-is from the Reloaded3 planning doc; and needs some refinements:
    
    - Extract Layer 2 stuff into `Virtual File Framework`
    - Extract Layer 3 stuff into `Archive Emulation Framework`
    - And make a page for Nx2VFS



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

#### Layer 1 Key APIs

**Public APIs** (via `Redirector`):

Individual File Redirects:

- **`add_file(&self, source_path: &str, target_path: &str) -> Result<RedirectHandle, VfsError>`**
    - Redirect individual file paths manually.

- **`remove_file(&self, handle: RedirectHandle) -> Result<(), VfsError>`**
    - Remove an individual file redirect.

Automatic File Redirects (Folder-as-Files):

- **`add_folder_as_files(&self, source_folder: &str, target_folder: &str) -> Result<FolderFilesHandle, VfsError>`**
    - Track current state of folder and create redirects automatically with `FileSystemWatcher`.
    - Used to map mod folders to game folders.
    - Supports real-time edits of content in mod folder.
    - **Recommended for most mod scenarios.**

- **`remove_folder_as_files(&self, handle: FolderFilesHandle) -> Result<(), VfsError>`**
    - Remove folder-as-files and associated file redirects.

Folder Fallback Redirects:

- **`add_folder(&self, source_folder: &str, target_folder: &str) -> Result<FolderRedirectHandle, VfsError>`**
    - Create folder fallback redirect (Tier 2 lookup).
    - **Use when you expect writes to non-mod (game) folders** (e.g., save files, config files).
    - If you expect writes to mod folders, use `add_folder_as_files()` instead.

- **`remove_folder(&self, handle: FolderRedirectHandle) -> Result<(), VfsError>`**
    - Remove a folder fallback redirect.

**Private APIs** (via `VirtualFiles`, for Layer 2 only):

- **`register_virtual_file(&self, file_path: &str, metadata: VirtualFileMetadata) -> Result<VirtualFileHandle, VfsError>`**
    - Make a virtual file visible in directory searches.
    - Layer 2 calls this to register virtual files so they appear when games search directories.

- **`unregister_virtual_file(&self, handle: VirtualFileHandle) -> Result<(), VfsError>`**
    - Remove a virtual file from directory search results.

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

These are public APIs for extensions (Layer 3):

- **`register_virtual_file(&self, path: &str, metadata: VirtualFileMetadata, file_handler: Box<dyn FileHandler>) -> Result<VirtualFileHandle, VfsError>`**
    - Allows extensions to create virtual files that Layer 1 will make visible in directory searches.
    - `metadata`: Immutable metadata about file (e.g. size)
    - `file_handler`: Object that implements methods for handling read operations
    - Internally calls Layer 1's `VirtualFiles::register_virtual_file()`

- **`unregister_virtual_file(&self, handle: VirtualFileHandle) -> Result<(), VfsError>`**
    - Removes a virtual file registered earlier.
    - Internally calls Layer 1's `VirtualFiles::unregister_virtual_file()`

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
- Added missing NtOpenFile API and other missing APIs, walked through entire kernel32 API for I/O, making graphs in this document.
- APIs use handles to unregister/dispose.
- Added Support for IoRing

And various other improvements all around detailed here.