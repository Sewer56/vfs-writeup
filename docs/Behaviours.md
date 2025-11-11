# Behaviours & Limitations

!!! warning "Work in Progress - Being Refined"

    This page is still being adjusted as finer details are refined during planning. Some information here may be outdated or incomplete.

!!! info "Understanding VFS constraints and edge cases"

    This page documents the limitations, edge cases, and unexpected behaviours of the Virtual FileSystem. Some are by design choice, others are fundamental technical constraints.

## Design Limitations (By Choice)

These are intentional design decisions that define what the VFS is and isn't meant to do.

### Read-Only Focus

!!! warning "Write behaviour not fully planned"

    This VFS design was originally for **reading redirected files** within the Reloaded3 environment, not complex write scenarios.
    
    The fine details of write behaviour have not yet been thoroughly planned, as scenarios with self-modifying games (via Tools, e.g. xEdit) have never been required in any games I (Sewer) have dealt with.
    
    I'll need to consult with more experienced folks such as the Mod Organizer 2 team to define such edge cases.
    
    Self-modifying games are really rare.

**As a general rule of thumb:**

- ✅ Reading files from mod folders works perfectly
- ⚠️ Write operations work but behaviour may change as details are refined
- ⚠️ Modifying redirected files modifies the mod's copy
- ⚠️ Deleting redirected files deletes the mod's copy
- ❌ Complex write semantics (copy-on-write, union mounts) not implemented

### Write Operation Behaviors

Write behaviour differs depending on which type of redirect is used.

#### Individual File Redirects (Tier 1)

For files redirected with `add_file()` or `add_folder_as_files()`:

| Operation              | Source Path     | Redirected To  | File Exists?                  | Actual Behavior                                                  | Result Location                                                                                          |
| ---------------------- | --------------- | -------------- | ----------------------------- | ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| **New File Creation**  | `game/file.txt` | `mod/file.txt` | N/A                           | Path redirected, creates new file                                | `mod/file.txt`                                                                                           |
| **Open/Read File**     | `game/data.pak` | `mod/data.pak` | `mod/data.pak` exists         | Opens file at redirected path                                    | `mod/data.pak`                                                                                           |
| **Open/Read File**     | `game/data.pak` | `mod/data.pak` | `mod/data.pak` does not exist | **Error** (file not found)                                       | N/A                                                                                                      |
| **Edit Existing File** | `game/data.pak` | `mod/data.pak` | `mod/data.pak` exists         | Modifies file at redirected path                                 | `mod/data.pak`                                                                                           |
| **Edit Existing File** | `game/data.pak` | `mod/data.pak` | `mod/data.pak` does not exist | **Error** (file not found)                                       | N/A                                                                                                      |
| **Delete File**        | `game/data.pak` | `mod/data.pak` | `mod/data.pak` exists         | Deletes file at redirected path                                  | `mod/data.pak` (deleted), original `game/data.pak` still exists (game will not see file due to redirect) |
| **Delete File**        | `game/data.pak` | `mod/data.pak` | `mod/data.pak` does not exist | **Error** (file not found)                                       | N/A                                                                                                      |
| **Delete & Recreate**  | `game/data.pak` | `mod/data.pak` | `mod/data.pak` exists         | Deletes `mod/data.pak`, creates new file (redirect still exists) | **Mod file deleted**, new file at `mod/data.pak` (redirect still active)                                 |
| **Copy File (source)** | `game/src.dat`  | `mod/src.dat`  | `mod/src.dat` exists          | `mod/src.dat` is copied to destination                           | `game/dest.dat` (or `mod/dest.dat` if destination is also redirected)                                    |
| **Copy File (source)** | `game/src.dat`  | `mod/src.dat`  | `mod/src.dat` does not exist  | **Error** (file not found)                                       | N/A                                                                                                      |
| **Move/Rename**        | `game/old.dat`  | `mod/old.dat`  | Either exists                 | Depends on API used                                              | Complex (see below)                                                                                      |

!!! question "Open Design Question: Delete Behaviour"

    When a file is deleted via VFS, should we:
    
    - **Delete only the redirected file** (current behaviour) - leaving the original `game/data.pak` intact, but the game will still see it since the redirect is gone
    - **Delete both original and redirected files** for consistency - so the game doesn't see a file it just deleted
    - **Pretend the original doesn't exist** - keep original file but mark it as hidden/deleted in VFS state
    
    Currently, deleting a redirected file only deletes the mod's copy, and the game will see the original file again. This may not be the desired behaviour.

!!! warning "FileSystemWatcher Does NOT Remove Redirects on File Deletion"

    By default, when using `add_folder_as_files()`, the `FileSystemWatcher` **does not** automatically remove redirects when it detects a file deletion in the mod folder.
    
    **Why?** This is intentional to handle the **delete-and-recreate pattern** used by many applications:
    
    1. Application deletes `mod/file.txt` (monitored by FileSystemWatcher)
    2. Application immediately recreates `mod/file.txt` with new content
    
    If we removed the redirect on deletion, step 2 would fail because the redirect wouldn't exist anymore and the new file would go to the wrong location.
    
    **Configurable behaviour:** You can enable redirect removal on file deletion by setting:
    
    ```rust
    settings.set_setting(VfsSetting::RemoveRedirectOnFileDelete, true);
    ```
    
    When enabled, FileSystemWatcher will remove redirects when files are deleted from the mod folder. Use this only if you're certain your applications don't use the delete-and-recreate pattern.
    
    **Default:** `false` (redirects persist after file deletion)

#### Folder Fallback Redirects (Tier 2)

For paths under folders registered with `add_folder()`:

| Operation              | Source Path            | Folder Redirect              | File Exists?                       | Actual Behavior                                          | Result Location                                                                                                  |
| ---------------------- | ---------------------- | ---------------------------- | ---------------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **New File Creation**  | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | N/A                                | Path redirected, creates new file                        | `mod/saves/save1.dat`                                                                                            |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Opens file at redirected path                            | `mod/saves/save1.dat`                                                                                            |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to original location                          | `game/saves/save1.dat`                                                                                           |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Edits file at redirected path                            | `mod/saves/save1.dat`                                                                                            |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to editing original                           | `game/saves/save1.dat`                                                                                           |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Deletes file at redirected path                          | `mod/saves/save1.dat` (deleted), `game/saves/save1.dat` may still exist (game will not see file due to redirect) |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to deleting original                          | `game/saves/save1.dat` (deleted)                                                                                 |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Delete & Recreate**  | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Either exists                      | Deletes whichever exists, creates at redirected location | Previous file deleted, new file at `mod/saves/save1.dat`                                                         |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | `mod/saves/src.dat` exists         | Copies from redirected path                              | `game/saves/dest.dat` (or `mod/saves/dest.dat` if destination is also redirected)                                |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | Only `game/saves/src.dat` exists   | Falls back to copying original                           | `game/saves/dest.dat` (or `mod/saves/dest.dat` if destination is also redirected)                                |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Move/Rename**        | `game/saves/old.dat`   | `game/saves/` → `mod/saves/` | Either exists                      | Depends on API used                                      | Complex (see below)                                                                                              |

!!! info "Folder fallback behaviour"

    Folder redirects apply to ALL operations including creation. The path is always redirected first:
    
    - For **reads/opens**: Check redirected location first (`mod/saves/`), fall back to original (`game/saves/`)
    - For **writes/creates**: Write to redirected location (`mod/saves/`)
    - For **deletes**: Delete from redirected location if file exists there, otherwise original

#### Delete-and-Recreate Pattern Warning

!!! danger "Delete-and-recreate will lose mod files"

    If a game or tool deletes a file and then recreates it with the same name, the behaviour depends on redirect type:
    
    **File Redirects (Tier 1) with `add_file()`:**
    
    1. Game deletes `game/file.txt` → VFS deletes `mod/file.txt` (**your mod file is gone**)
    2. Game creates `game/file.txt` → Redirect still exists, creates at `mod/file.txt` (redirected location)
    
    **Result**: Your mod's file is deleted and recreated as a new empty/different file at `mod/file.txt`.
    
    **File Redirects (Tier 1) with `add_folder_as_files()`:**
    
    By default (with `RemoveRedirectOnFileDelete = false`):
    
    1. Game deletes `game/file.txt` → VFS deletes `mod/file.txt` (**your mod file is gone**)
    2. Game creates `game/file.txt` → Redirect still exists, creates at `mod/file.txt` (redirected location)
    
    **Result**: Your mod's file is deleted and recreated as a new empty/different file at `mod/file.txt`.
    
    With `RemoveRedirectOnFileDelete = true`:
    
    1. Game deletes `game/file.txt` → VFS deletes `mod/file.txt` (**your mod file is gone**)
    2. FileSystemWatcher detects the mod file deletion and removes the redirect
    3. Game creates `game/file.txt` → Redirect no longer exists, creates at `game/file.txt` (original location)
    
    **Result**: Your mod's file is deleted, new file is in game folder.
    
    **Folder Fallback (Tier 2):**
    
    1. Game deletes `game/saves/file.txt` → VFS deletes `mod/saves/file.txt` (if it exists) or `game/saves/file.txt`
    2. Game creates `game/saves/file.txt` → Creates at `mod/saves/file.txt` (redirected location)
    
    **Result**: Deleted file is gone, new file is created at redirected location (`mod/saves/`).

#### Move/Rename Behavior

Move and rename operations are complex because they involve both source and destination paths:

- **Source path**: May be redirected (Tier 1/2 lookup)
- **Destination path**: May or may not be redirected (separate lookup)

Depending on how the OS API is used:

- **NtSetInformationFile with FileRenameInformation**: Renames/moves the file at the redirected source location
- **MoveFileEx with MOVEFILE_COPY_ALLOWED**: May copy from redirected source to final destination
- **Cross-volume moves**: Always trigger copy + delete pattern

**Examples:**

| Redirect Type   | Source                                           | Destination                     | Result                                          |
| --------------- | ------------------------------------------------ | ------------------------------- | ----------------------------------------------- |
| File redirect   | `game/old.txt` → `mod/old.txt`                   | `game/new.txt` (not redirected) | Renames/moves `mod/old.txt` to `game/new.txt`   |
| Folder fallback | `game/saves/old.sav` → `mod/saves/old.sav`       | `game/saves/new.sav`            | Renames within redirected folder structure      |
| Folder-as-files | `game/textures/old.png` → `mod/textures/old.png` | `game/textures/new.png`         | FileSystemWatcher detects deletion and creation |

!!! warning "Folder renames are unpredictable"

    Renaming folders containing redirected files may move either:
    
    - The original (empty) folder, or
    - The files from both original and mod folders, or
    - Only the files covered by redirects
    
    Behavior depends on which Windows API the application uses and which redirect types are active.

### Per-Process Scope & Child Processes

!!! info "Initial release is per-process only"

The VFS operates **per-process**. Each process that wants VFS must initialize it independently.

**Implications:**

- ✅ Safe: VFS won't affect other programs on your system
- ✅ Isolated: Each process has independent VFS state
- ❌ Child processes don't inherit VFS state
- ❌ Separate applications can't see each other's redirects
- ❌ Not suitable for system-wide virtualization

#### Child Process Support

!!! note "Child process support: Not in initial release"

    **Initial release:** Per-process only. Child processes will not see virtualized files.
    
    **Future:** Child process injection will be implemented when needed/required.

If a game launches external tools or helper processes, those processes **will not see the virtualized filesystem** in the initial release.

**Example scenario:**

```
Game.exe (VFS active, sees mod files)
  └─ launches Tool.exe (no VFS, sees only original files)
```

**Future implementation:** Automatic injection into child processes when this becomes a requirement.

!!! tip "This is a very rare requirement"

    Child process support is rarely needed. It's primarily required for modding tools that operate on a pre-modded game folder (e.g., tools that launch the game as a child process to test changes).
    
    Regular games launching helper processes for replay rendering, screenshot processing, etc. typically don't need those processes to see modded files.

## Memory-Mapped Files

!!! warning "Virtual files have limited memory mapping support"

Virtual files (Layer 2) need special handling for memory-mapped I/O.

**Current Idea:**

- ✅ Regular file I/O: Fully supported
- ⚠️ Memory mapping (mmap): Requires Layer 2 implementation
- 📝 Small mappings (<128KB): Can be pre-populated
- 📝 Large mappings: Page fault handling can (to my knowledge) be emulated with `VectoredExceptionHandler(s)`, but I need to test in practice
- 📝 Write tracking: For writes we have `GetWriteWatch` & `ResetWriteWatch`

**Platform APIs affected:**

- `CreateFileMapping`, `MapViewOfFile`, `NtCreateSection`, `NtMapViewOfSection`

**Impact:** Low - very few games use memory-mapped I/O. Disk I/O is traditionally preferred for game assets due to optical disc heritage.

**Implementation status:** Architecture defined, not yet implemented.

## Technical Limitations

These are fundamental constraints based on how the VFS operates.

### File Permissions & ACLs

!!! info "Not handled by the VFS"

The VFS does **not manage or check file permissions** or Access Control Lists (ACLs).

**Assumption:** The user running the game has read access to both original and mod files.

**What this means:**

- ❌ No permission/ACL spoofing for virtual files
- ❌ No support for Windows security descriptors
- ❌ No support for Unix file mode bits (chmod/chown)
- ✅ Files use their actual on-disk permissions

**Impact:** Negligible - game stores don't use ACLs, and mods are installed by the user (who has access).

**Platform APIs not hooked:**

- Windows: `GetFileSecurity`, `SetFileSecurity`, `GetSecurityInfo`, `SetSecurityInfo`
- Linux: `chmod`, `fchmod`, `chown`, `fchown`, `setxattr` (for security attributes)

### File Encryption

!!! info "Not supported"

NTFS file encryption (EFS) is not supported for virtual files.

**Platform APIs not hooked:**

- `EncryptFile`, `DecryptFile`, `FileEncryptionStatus`
- `OpenEncryptedFileRaw` (raw encrypted file access)

**Impact:** None - game stores don't support encrypted files, legacy games don't use them.

### NTFS Alternate Data Streams

!!! info "Not supported"

NTFS Alternate Data Streams (ADS) are not supported for virtualized paths.

**Example ADS syntax:** `file.txt:hidden_stream`

**Platform APIs not hooked:**

- `FindFirstStreamW`, `FindNextStreamW`

**Impact:** None - game stores don't support ADS. Modern games don't use them.

### DOS 8.3 Short Names

!!! info "Not returned for virtual files"

Virtual files do not have DOS 8.3 short names (like `PROGRA~1` for `Program Files`).

**Platform APIs affected:**

- `GetShortPathName` - returns long path for virtual files
- `SetFileShortName` - not hooked

**Impact:** None - modern games don't use DOS short names.

### NTFS Compression

!!! info "Reported as uncompressed"

Virtual files report as uncompressed, even if backing file is NTFS-compressed.

**Platform APIs affected:**

- `GetCompressedFileSize` - returns actual file size, not compressed size

**Impact:** Negligible - only affects disk space reporting. Game functionality unaffected.

### File Locking

!!! info "Locking operates on redirected files"

File locking operations (`LockFile`, `LockFileEx`) work on the **redirected file**, not the original.

**Platform APIs:** `LockFile`, `LockFileEx`, `UnlockFile`, `UnlockFileEx`

**Impact:** Minimal - games rarely use file locking. When they do, locking the redirected file is usually the desired behaviour.

### Extended Attributes

!!! info "Extended attributes not virtualized"

Linux extended attributes (xattrs) and Windows extended attributes are not virtualized.

**Platform APIs not hooked:**

- Linux: `setxattr`, `getxattr`, `listxattr`, `removexattr` families
- Windows: `NtQueryEaFile`, `NtSetEaFile`

**Impact:** None - games don't use extended attributes.

### Transactional NTFS

!!! info "Deprecated Windows feature - works transparently"

Transactional NTFS (TxF) is a deprecated Windows Vista feature. It's wrapped around the standard APIs we already hook, so it works transparently.

**Platform APIs (deprecated):**

- `CreateFileTransactedA/W`, `CopyFileTransactedA/W`, etc.

**Impact:** None - never seen a program use this. Microsoft deprecated it in Windows 8 (2012).

### Reparse Points & Symbolic Links

!!! info "Reparse point tags not supported"

Virtual files do not support NTFS reparse point tags (symbolic links, mount points, etc.).

**What works:**

- ✅ Redirecting **to** a symlink target works (VFS sees the target path)
- ✅ Real symlinks in mod folders work normally

**What doesn't work:**

- ❌ Virtual files can't pretend to be symlinks
- ❌ No support for custom reparse point types

**Impact:** Minimal - doesn't affect mods stored on OneDrive/cloud storage (they use different mechanisms, not reparse points).

## Platform-Specific Limitations

### Windows: UWP/WinRT Apps

!!! warning "Pure UWP apps may not work"

**Desktop Bridge a.k.a. Project Centennial:** This is Win32 apps packaged with a UWP wrapper. These apps declare `runFullTrust` capability in their manifest and have full filesystem access like regular Win32 programs. Basically all games on Xbox Store use this approach.

**Standard sandboxed UWP apps:** True UWP apps without `runFullTrust` have restricted filesystem access and may use brokered API calls through `RuntimeBroker.exe`.

**What works:**

- ✅ Desktop Bridge apps (basically all games on Xbox Store)

**What might not work:**

- ⚠️ Pure UWP apps without `runFullTrust` capability

!!! note "Additional considerations for pure UWP"

    I have not yet investigated if we can simply add `runFullTrust` to regular UWP programs to make them work. Never ran into a real UWP game.
    
    Even if VFS hooks work, accessing the game folder would require dealing with the classic read/write restrictions of the `WindowsApps` folder for non-Win32 apps.

**Impact:** Negligible - basically all games on Xbox Store use Desktop Bridge (or they can't ship to any other game store).

### Windows: System Calls vs. ntdll.dll

!!! info "VFS hooks ntdll.dll, not system calls"

The VFS hooks `ntdll.dll` functions, not direct system calls.

**Why:**

- Windows syscall numbers are **unstable** between versions
- All normal user-mode software goes through `ntdll.dll`
- Direct syscalls are extremely rare (anti-cheat, debuggers, malware)

**What this means:**

- ✅ Works with 99.9% of software (everything using Win32/CRT/STL)
- ❌ Won't work with software that directly issues syscalls bypassing ntdll.dll

**Impact:** None for games - games use standard APIs.

If they didn't, they'd be broken on the next version of Windows.

### Missing APIs (Wine & Older Windows)

!!! warning "The latest and greatest APIs aren't available everywhere"

The latest and greatest Windows APIs may not be available on older Windows versions (e.g., 24H2 APIs unavailable on 23H2) or Wine.

**Missing APIs (as of November 2025):**

- `NtQueryInformationByName` (Windows 10 1703+, used by `GetFileInformationByName` which was added in 24H2)

**Workaround:** Don't hook functions that don't exist.

**Impact:** None.

### API Wrapping & Re-Entry Prevention

!!! info "In very rare cases, ntdll APIs may be replaced with Ex variants"

    In very rare cases, APIs in ntdll may be replaced with Ex variants internally. When this happens, the original non-Ex variant will call the Ex variant. For performance and correctness reasons, it is preferable to avoid VFS hooks firing twice for a single operation when hooking both.

**Known replacement cases (affects both Windows and Wine):**

- `NtQueryDirectoryFile` → `NtQueryDirectoryFileEx` (Windows 10 1709+)
- `NtNotifyChangeDirectoryFile` → `NtNotifyChangeDirectoryFileEx` (version unidentified)

When the replacement occurs, hooking both APIs would invoke VFS twice for the same operation:

```
Application calls NtQueryDirectoryFile
  → VFS hook intercepts (1st entry)
    → Calls original NtQueryDirectoryFile
      → ntdll internally calls NtQueryDirectoryFileEx (replacement)
        → VFS hook intercepts again (2nd entry - undesirable)
```

**Solution:** VFS uses a thread-local exclusion mechanism to detect and skip the redundant second entry when the Ex variant is called from within the base API.

Did this before and my code worked in Windows 7, Windows 10 and Wine just fine, tests included.

**Impact:** None - handled transparently by the VFS.

### Linux: Not Yet Implemented

!!! info "Native Linux support is planned but not implemented"

Linux native support requires syscall patching implementation.

**Status:**

- ✅ Baseline exploration started - looked at the basics
- ❌ Implementation not started
- ⏳ Will be added when community demand exists

**Approach:** Disassemble loaded libraries to find syscall instructions and patch with jumps to hook functions. See [index.md](index.md#linux) for detailed approach.

**Complexity:** Per-architecture work (x86_64, AArch64, etc.), ~1 day per architecture after first one.

## Edge Cases & Unexpected Behaviors

### Case Sensitivity

!!! info "Platform-dependent behaviour"

**Windows:**

- Case-insensitive by default (NTFS, FAT32)
- VFS uses case-insensitive comparisons
- `Game.txt` and `game.txt` are the same file

**Linux:**

- Case-sensitive by default (ext4, btrfs, etc.)
- VFS uses case-sensitive comparisons
- `Game.txt` and `game.txt` are different files

**Impact:** Cross-platform mods need to use consistent casing.

### Path Separators

!!! info "Automatically normalized"

The VFS normalizes path separators:

- Windows: Accepts both `/` and `\`, converts to `\` internally
- Linux: Only `/` is valid (no backslashes in paths)

**Impact:** None - handled automatically.

### Long Paths (Windows)

!!! warning "Windows 260-character path limit"

Windows has a 260-character path limit (`MAX_PATH`) for many APIs.

**VFS behaviour:**

- Uses NT-style paths (`\\?\C:\...`) internally to support long paths
- Applications using short-path APIs limited to 260 characters
- Applications using long-path APIs can use full 32,767 characters

**Impact:** Minimal - most games use standard paths well under 260 characters.

### Unicode Normalization

!!! info "No normalization performed"

The VFS does **not normalize Unicode strings**. Paths must match byte-for-byte.

**Example:** `café` (NFC) vs `café` (NFD) are treated as different paths.

**Impact:** Rare - most applications use one normalization form consistently.

### Handle Lifetime

!!! warning "Handles remain valid after redirect removal"

Once a file handle is opened (redirected), it **remains valid** even if the redirect is removed.

**Example with file redirects:**

```rust
// Add redirect
let handle = redirector.add_file("game/file.txt", "mod/file.txt")?;

// Game opens the file (gets handle to mod/file.txt)
let file = open("game/file.txt"); // redirected to mod/file.txt

// Remove redirect
redirector.remove_file(handle)?;

// File handle is still valid and points to mod/file.txt
read(file); // still reads from mod/file.txt
```

**Example with folder-as-files:**

```rust
// Add folder as files (creates redirects for all files)
let handle = redirector.add_folder_as_files("game/data", "mod/data")?;

// Game opens a file (gets handle to mod/data/file.txt)
let file = open("game/data/file.txt"); // redirected to mod/data/file.txt

// Remove folder-as-files (removes all file redirects)
redirector.remove_folder_as_files(handle)?;

// File handle is still valid and points to mod/data/file.txt
read(file); // still reads from mod/data/file.txt
```

**Why:** File handles are OS-level resources pointing to the actual opened file.

**Impact:** Generally desired behaviour - removing a redirect shouldn't break open files.

### Concurrent Modifications

!!! warning "FileSystemWatcher delays (folder-as-files only)"

The `FileSystemWatcher` used by `add_folder_as_files()` may have slight delays in detecting changes to the mod folder.

**Typical delay:** <100ms on local filesystems

**Limitations:**

- ⚠️ Network filesystems may have longer delays
- ⚠️ Some filesystems don't support change notifications (old NFS, some remote mounts)
- ⚠️ High-frequency changes may be coalesced
- ⚠️ Folder renames may not be detected properly

**What this means:**

- `add_file()`: No watcher, changes to mod files not tracked automatically
- `add_folder_as_files()`: Watcher active, changes detected within ~100ms
- `add_folder()`: No watcher, folder contents resolved at access time (always current)

**Impact:** Minimal - delays are typically imperceptible during gameplay.

### Redirect Priority

!!! info "Lookup order and redirect priority"

The VFS checks redirects in a specific order:

**Tier 1: File-level redirects (checked first)**

- Individual file redirects from `add_file()`
- File redirects created by `add_folder_as_files()`
- Within Tier 1: **Later redirects override earlier ones**

**Tier 2: Folder fallback redirects (checked if Tier 1 fails)**

- Folder redirects from `add_folder()`
- Multiple folder redirects can apply (most specific path wins)

**Example - Tier 1 priority:**

```rust
redirector.add_file("game/file.txt", "mod1/file.txt")?;
redirector.add_file("game/file.txt", "mod2/file.txt")?;
// Opens mod2/file.txt (later redirect wins)

redirector.add_folder_as_files("game/data", "mod3/data")?; // Creates redirect for game/data/file.txt
// If mod3/data/file.txt exists, it now redirects to mod3/data/file.txt (latest Tier 1 redirect)
```

**Example - Tier 1 vs Tier 2:**

```rust
redirector.add_folder("game/saves", "mod/saves")?;        // Tier 2
redirector.add_file("game/saves/file.sav", "mod2/file.sav")?;  // Tier 1

// Opens mod2/file.sav (Tier 1 checked first)
open("game/saves/file.sav");

// For files NOT explicitly redirected in Tier 1:
open("game/saves/other.sav"); // Falls back to Tier 2: mod/saves/other.sav
```

**Impact:** Intentional design - allows mod priority/load order systems.

### Directory Moves During Runtime

!!! danger "Moving mod folders during runtime is undefined"

If a mod folder is moved or renamed while the game is running, behaviour depends on redirect type:

**File redirects (`add_file()`):**

- ❌ Break immediately (hardcoded path no longer valid)
- Open file handles remain valid but point to old location
- New file opens will fail (path doesn't exist)

**Folder-as-files (`add_folder_as_files()`):**

- ⚠️ `FileSystemWatcher` may or may not detect the folder move
- If detected: All file redirects are removed
- If not detected: Redirects break (paths invalid)
- Open file handles remain valid but point to old location

**Folder fallback (`add_folder()`):**

- ❌ Break immediately (hardcoded path no longer valid)
- Lookups will fall back to original location
- Open file handles remain valid but point to old location

**Recommendation:** Don't move mod folders while the game is running.

## Incompatible Tools & Workflows

### Modding Tools That Modify Game Folder

!!! danger "Tools expecting to modify the game folder directly"

Some modding tools are designed to operate on the actual game folder and expect to create/modify/delete files there.

**Examples:**

- **Skyrim xEdit**: Expects to create/modify plugin files in game's `Data` folder
- **Fallout 4 Creation Kit**: Expects to write directly to game folder
- **Asset extractors** that write to game folder

**Problem:**

1. Tool writes to `game/newfile.esp`
2. VFS may redirect this to `mod/newfile.esp` (if folder redirect exists)
3. Tool expects file at `game/newfile.esp` but it's actually at `mod/newfile.esp`
4. Confusion and errors

**Solution:** **Disable VFS when running these tools.** Use VFS for the game itself, not for modding tools.

### Delete-and-Recreate Workflows

!!! danger "Tools that delete and recreate files"

Some tools use a delete-and-recreate pattern for safe file updates:

1. Read original file
2. Delete original file
3. Write new file with same name

**Problem:** As documented above, this deletes your mod's file and creates a new file in the original location.

**Solution:** Don't use VFS with these tools.

### Cross-Location Operations

!!! warning "Moving/copying files between redirected and non-redirected locations"

Moving or copying files between redirected paths and original paths may have unexpected results.

**Examples:**

**File redirect to non-redirected:**
```
Source: game/old.txt → mod/old.txt (file redirect)
Dest: game/new.txt (not redirected)

Move operation: Moves/copies from mod/old.txt to game/new.txt
```

**Folder fallback to non-redirected:**
```
Source: game/saves/old.sav → mod/saves/old.sav (folder fallback)
Dest: game/backups/old.sav (not redirected)

Move operation: Moves/copies from mod/saves/old.sav to game/backups/old.sav
```

**Between different folder redirects:**
```
Source: game/saves/file.sav → mod1/saves/file.sav (folder redirect 1)
Dest: game/backups/file.sav → mod2/backups/file.sav (folder redirect 2)

Move operation: Moves/copies from mod1/saves/file.sav to mod2/backups/file.sav
```

**Impact:** Usually works as expected, but be aware that the actual source/destination are the redirected locations, not the game paths.

## Performance Considerations

While not limitations, these behaviours affect performance:

### Folder Fallback Performance

!!! info "Tier 2 redirects are slower than Tier 1"

**Lookup time by redirect type:**

- **`add_file()`** (Tier 1): O(1) hash lookup - fastest
- **`add_folder_as_files()`** (Tier 1 internally): O(1) hash lookup - fastest
- **`add_folder()`** (Tier 2): O(N) path component matching (N = path depth) - slower

### Optimize() Required for Best Performance

!!! tip "Call optimize() after adding redirects"

After adding many redirects, call `optimize()` to build the fast `LookupTree` structure.

**Without optimize():** O(N) lookup (path depth)  
**With optimize():** O(3) lookup (prefix + subfolder + filename)

### Memory Scales with Directory Count

!!! info "Memory usage driven by directory structure"

Memory usage scales primarily with **directory count**, not file count, due to string pooling.

Approximately **~64 bytes per file** on average (in original C# implementation), based on Steam games folder. We can still do better. See [Performance](Virtual-FileSystem/Performance.md#file-mapping-performance-memory-usage) for detailed benchmarks.

## Summary: When NOT to Use VFS

!!! danger "VFS is not appropriate for:"

- ✏️ **Modding tools that modify game folder** (xEdit, Creation Kit, etc.) - use VFS for game, not tools
- 🗑️ **Delete-and-recreate workflows** - will lose files from mod folders
- 👶 **Child process workflows** - external tools won't see virtualized files
- 🔐 **Security isolation** - VFS is not a security boundary
- 💼 **System-wide virtualization** - works per-process only
- 🌐 **Network filesystem reliance** - FileSystemWatcher may be unreliable
- 📝 **Tools requiring ACLs/permissions** - not supported
- 🔒 **Encrypted file support** - NTFS EFS not supported
- 💾 **NTFS advanced features** - ADS, reparse points, compression not supported

!!! success "VFS is perfect for:"

- 🎮 **Game modding** - replace/add game assets transparently
- 📦 **Archive replacement** - redirect game archives without extraction
- 💾 **Save file redirection** - move saves to different locations
- ⚙️ **Config file management** - per-mod config overrides
- 🔄 **Hot-reload development** - edit files and see changes immediately
