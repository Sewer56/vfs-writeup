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

### Write Operation Behaviours

Write behaviour differs depending on which type of redirect is used.

#### Individual File Redirects (Tier 1)

For files redirected with `add_file()` or `add_folder_as_files()`:

| Operation              | Source Path     | Redirected To  | File Exists?                  | Actual Behaviour                                                 | Result Location                                                                                          |
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

| Operation              | Source Path            | Folder Redirect              | File Exists?                       | Actual Behaviour                                         | Result Location                                                                                                  |
| ---------------------- | ---------------------- | ---------------------------- | ---------------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **New File Creation**  | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | N/A                                | Path redirected, creates new file                        | `mod/saves/save1.dat`                                                                                            |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Opens file at redirected path                            | `mod/saves/save1.dat`                                                                                            |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to original location                          | `game/saves/save1.dat`                                                                                           |
| **Open/Read File**     | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Edits file at redirected path                            | `mod/saves/save1.dat`                                                                                            |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to editing original                           | `game/saves/save1.dat`                                                                                           |
| **Edit Existing File** | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | `mod/saves/save1.dat` exists       | Deletes file at redirected path                          | `mod/saves/save1.dat` (deleted), sticky redirect persists (file not found on subsequent access)                   |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Only `game/saves/save1.dat` exists | Falls back to deleting original                          | `game/saves/save1.dat` (deleted)                                                                                 |
| **Delete File**        | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Delete & Recreate**  | `game/saves/save1.dat` | `game/saves/` → `mod/saves/` | Either exists                      | Deletes whichever exists, creates at redirected location | Previous file deleted, new file at `mod/saves/save1.dat`                                                         |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | `mod/saves/src.dat` exists         | Copies from redirected path                              | `game/saves/dest.dat` (or `mod/saves/dest.dat` if destination is also redirected)                                |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | Only `game/saves/src.dat` exists   | Falls back to copying original                           | `game/saves/dest.dat` (or `mod/saves/dest.dat` if destination is also redirected)                                |
| **Copy File (source)** | `game/saves/src.dat`   | `game/saves/` → `mod/saves/` | Neither exists                     | **Error** (file not found)                               | N/A                                                                                                              |
| **Move/Rename**        | `game/saves/old.dat`   | `game/saves/` → `mod/saves/` | Either exists                      | Depends on API used                                      | Complex (see below)                                                                                              |

!!! warning "Sticky Redirects Required for Folder Fallback"

    **Design issue:** Plain folder redirection with dynamic lookup has inconsistent deletion behaviour.
    
    **Problem with naive implementation:**
    
    Normally, out of the box with plain folder redirection, we'd get this behaviour:
    
    1. Both `game/saves/save1.dat` (original) and `mod/saves/save1.dat` (modded) exist
    2. Game opens `game/saves/save1.dat` → reads from `mod/saves/save1.dat` ✓
    3. Something deletes `mod/saves/save1.dat`
    4. Game opens `game/saves/save1.dat` → **now reads from `game/saves/save1.dat`** (original becomes visible!)
    
    This happens because plain folder redirection performs dynamic lookup on every access: "Does mod file exist? No → Fall back to original location."
    
    **Solution: Implement sticky redirects**
    
    When we redirect `game/saves/save1.dat` → `mod/saves/save1.dat` through a folder redirect, we need to **remember that specific file mapping** and keep it alive as long as the folder redirect exists.
    
    **Sticky redirect behaviour:**
    
    1. First access to `game/saves/save1.dat` finds `mod/saves/save1.dat` (exists)
    2. **Create persistent mapping:** `game/saves/save1.dat` → `mod/saves/save1.dat` (sticky)
    3. Something deletes `mod/saves/save1.dat`
    4. Game opens `game/saves/save1.dat` → redirect still points to `mod/saves/save1.dat` → **File not found** (consistent with Tier 1)
    
    This makes folder redirects behave consistently with `add_file()` and `add_folder_as_files()`, where redirects persist regardless of file existence.
    
    **Why this matters:**
    
    Modding tools that operate on a pre-modded game folder (xEdit, Creation Kit, etc. - see [below](#modding-tools-that-modify-game-folder)) may potentially delete and recreate files. Without sticky redirects:
    
    - Tool deletes modded file → original becomes visible
    - Tool expects to work with empty/deleted state → instead sees stale original data
    - Can lead to unexpected behaviour
    
    **Implementation approach:** We can probably implement this by adding a file redirect (Tier 1) whenever a folder redirect successfully resolves to an existing file.
    
    **Needs consultation:** I (Sewer) should check with the Mod Organizer 2 folks on this - they've dealt with these tools for years and know all the edge cases.

!!! info "Folder fallback behaviour"

    Folder redirects apply to ALL operations including creation. The path is always redirected first:
    
    - For **reads/opens**: Check redirected location first (`mod/saves/`), fall back to original (`game/saves/`)
    - For **writes/creates**: Write to redirected location (`mod/saves/`)
    - For **deletes**: Delete from redirected location if file exists there, otherwise original

#### Move/Rename Behaviour

Move and rename operations are complex because they involve both source and destination paths:

- **Source path**: May be redirected (Tier 1/2 lookup)
- **Destination path**: May or may not be redirected (separate lookup)

Depending on how the OS API is used:

- **NtSetInformationFile with FileRenameInformation**: Renames/moves the file at the redirected source location
- **MoveFileEx with MOVEFILE_COPY_ALLOWED**: May copy from redirected source to final destination
- **Cross-volume moves**: Always trigger copy + delete pattern

**Examples:**

| Redirect Type   | Source                                     | Destination                     | Result                                         |
| --------------- | ------------------------------------------ | ------------------------------- | ---------------------------------------------- |
| File redirect   | `game/old.txt` → `mod/old.txt`             | `game/new.txt` (not redirected) | Renames/moves `mod/old.txt` to `game/new.txt`  |
| Folder fallback | `game/saves/old.sav` → `mod/saves/old.sav` | `game/saves/new.sav`            | Renames within redirected folder structure     |

!!! warning "Folder renames are unpredictable"

    Renaming folders containing redirected files may move either:
    
    - The original (empty) folder, or
    - The files from both original and mod folders, or
    - Only the files covered by redirects
    
    Behaviour depends on which Windows API the application uses and which redirect types are active.

### Per-Process Scope & Child Processes

!!! info "Per-process isolation"

The VFS operates **per-process**. Each process that wants VFS must initialize it independently.

If a game launches external tools or helper processes, those processes **will not see the virtualized filesystem** unless VFS is explicitly injected into them.

**Example scenario:**

```
Game.exe (VFS active, sees mod files)
  └─ launches Tool.exe (no VFS, sees only original files)
```

!!! tip "This is a very rare requirement"

    Child process support is rarely needed. It's primarily required for modding tools that operate on a pre-modded game folder (e.g., tools that launch the game as a child process to test changes).
    
    Regular games launching helper processes for replay rendering, screenshot processing, etc. typically don't need those processes to see modded files.

#### Requirements for Child Process Hooking

To automatically inject VFS into child processes, hook `NtCreateProcess` (or similar) - the lowest level user-mode API for process creation.

**Architecture transition support:**

| Parent Process | Child Process | Support                                              |
| -------------- | ------------- | ---------------------------------------------------- |
| x86            | x86           | ✅ Direct injection                                  |
| x64            | x64           | ✅ Direct injection                                  |
| x64            | x86           | ✅ Direct injection (WOW64)                          |
| x86            | x64           | ⚠️ Requires external 64-bit injector EXE             |
| ARM64          | x86/x64       | ❓ Get me an ARM64 device and I'll figure it out     |

## Memory-Mapped Files

!!! info "Investigating this is a work in progress"

    Determining the best approach for memory-mapped file support is still under investigation.

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

## DirectStorage

!!! info "TODO: Document DirectStorage considerations"

    DirectStorage is a Windows API for high-speed asset loading that bypasses the Win32 API.

TODO: Document DirectStorage compatibility. From previous observations, DirectStorage makes calls to ntdll that should work fine with VFS hooks. Worst case, DirectStorage can be forced to fall back to standard APIs.

## DLL Hijacking Load Order

!!! info "TODO: Document DLL hijacking considerations"

    DLL hijacking is a technique where a DLL is placed in a specific location to override system DLLs.

TODO: Document adding an import for the VFS DLL/Mod Loader in the PE header, and verify it works in both Windows and Wine.

## Unsupported Features

!!! info "Purpose-built for game modding"

    These are features deliberately not implemented because games don't use them.
    
    The VFS is purpose-built for game modding, not as a general-purpose filesystem virtualisation solution. There are no technical barriers to supporting them, just no need at the moment. If they're needed, they'll be added.

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

**Impact:** None - returning real file size is good enough.

### File Locking

!!! info "Locking operates on redirected files"

File locking operations (`LockFile`, `LockFileEx`) work on the **redirected file**, not the original.

**Platform APIs:** `LockFile`, `LockFileEx`, `UnlockFile`, `UnlockFileEx`

**Impact:** None - games have no reason to lock other processes from access. But even if they did, the behaviour is correct.

### Extended Attributes

!!! info "Extended attributes not virtualized"

Linux extended attributes (xattrs) and Windows extended attributes are not virtualized.

**Examples:**

- Linux: `user.comment`, `security.selinux`, `security.capability`, `trusted.*`
- Windows: DOS-era extended attributes (file comments, custom metadata)

**Platform APIs not hooked:**

- Linux: `setxattr`, `getxattr`, `listxattr`, `removexattr` families
- Windows: `NtQueryEaFile`, `NtSetEaFile`

**Impact:** None - games don't use extended attributes.

### Transactional NTFS

!!! info "Deprecated Windows feature"

Transactional NTFS (TxF) is a deprecated Windows Vista feature.

**Platform APIs (deprecated):**

- `CreateFileTransactedA/W`, `CopyFileTransactedA/W`, etc.

**Impact:** None - never seen a program use this. Microsoft deprecated it in Windows 8 (2012). We don't explicitly support it, but it should work out of the box since it wraps around the standard APIs we already hook.

### Reparse Points & Symbolic Links

!!! info "Reparse point tags not supported"

Virtual files do not support NTFS reparse point tags (symbolic links, mount points, etc.).

**What works:**

- ✅ Redirecting **to** a symlink target works (VFS sees the target path)
- ✅ Real symlinks in mod folders work normally

**What doesn't work:**

- ❌ Virtual files can't pretend to be symlinks
- ❌ No support for custom reparse point types

**Impact:** None - we can't pretend virtual files are symlinks, but games have no reason to check for this. For reparse points, cloud storage (e.g. OneDrive) uses different mechanisms and works fine.

## Platform-Specific Limitations

### Windows: UWP/WinRT Apps

!!! warning "Pure UWP apps may not work"

**Desktop Bridge a.k.a. Project Centennial:** This is Win32 apps packaged with a UWP wrapper. These apps declare `runFullTrust` capability in their manifest and have full filesystem access like regular Win32 programs. Almost all games on Xbox Store use this approach.

**Standard sandboxed UWP apps:** True UWP apps without `runFullTrust` have restricted filesystem access and may use brokered API calls through `RuntimeBroker.exe`.

**What works:**

- ✅ Desktop Bridge apps (almost all games on Xbox Store)

**What might not work:**

- ⚠️ Pure UWP apps without `runFullTrust` capability

**Impact:** Negligible - almost all games on Xbox Store use Desktop Bridge. Pure UWP apps are Microsoft Store(s) exclusive, as no other stores support them.

Pure UWP games would be a slice of the titles found [here](https://www.pcgamingwiki.com/wiki/List_of_games_exclusive_to_Microsoft_Store) - mostly mobile apps. Only notable entries are Microsoft Studios titles from late 2010s: Forza Horizon 3 (delisted years ago), Gears of War 4, Gears of War: Ultimate Edition. These titles are laced with encrypted filesystems, anti-code injection/cheat protections, etc.

!!! note "Additional considerations for pure UWP"

    I have not yet investigated if we can simply add `runFullTrust` to regular UWP programs to make them work. Never ran into a real UWP game.
    
    Even if VFS hooks work, accessing the game folder would require dealing with the classic read/write restrictions of the `WindowsApps` folder for non-Win32 apps.

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

## Edge Cases & Unexpected Behaviours

### Case Sensitivity

!!! info "VFS uses case-insensitive comparisons"

**Windows:**

- Case-insensitive by default (NTFS, FAT32)
- VFS uses case-insensitive comparisons
- `Game.txt` and `game.txt` are the same file

**Linux:**

- Case-sensitive by default (ext4, btrfs, etc.)
    - Some filesystems (e.g. ext4) can be set case-insensitive, but we can't rely on this
- VFS uses case-insensitive comparisons
    - Only one casing of a file is recognised
    - Users cannot be trusted to use correct case in cross-platform games (e.g. .NET)
- `Game.txt` and `game.txt` are treated as the same file

**Impact:** Avoid creating files with different casings of the same name - only one will be recognised.

### Path Separators

!!! info "Path separators are normalized to the native path separator"

The VFS normalizes path separators:

- Windows: Accepts both `/` and `\`, converts to `\` internally
- Linux: Only `/` is valid (no backslashes in paths)

**Impact:** None - handled automatically.

### Long Paths (Windows)

!!! info "Automatic long path handling"

Windows has a 260-character path limit (`MAX_PATH`) for many APIs.

**VFS behaviour:**

- Applications using `\??\<drive>:\...` prefix are correctly handled
- VFS automatically prepends `\??\` prefix if redirected path exceeds 260 characters
- Applications can use full 32,767 character paths without modification

!!! note "`\??\` not `\\?\`"

    We use `\??\` because we're talking about the ntdll namespace prefix.
    
    `\\?\` is the Win32 API prefix that gets converted to `\??\` by the time it reaches ntdll.

**Impact:** None - long paths are handled transparently.

### Unicode Normalization

!!! info "No normalization performed"

The VFS does **not normalize Unicode strings**. Paths must match byte-for-byte.

**Example:** `café` (NFC) vs `café` (NFD) are treated as different paths.

**Impact:** Negligible - in 2025, many games don't even properly support UTF-8 or Unicode at all, let alone use characters where normalization matters.

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

**Impact:** Negligible.

!!! info "Expected behaviour"

    Under normal circumstances, redirections are not removed while files are currently being accessed.
    
    Exception: Mod authors making live changes to their mod folder while the game is running (handled by `add_folder_as_files()` with `FileSystemWatcher`) do so at their own risk.

### Concurrent Modifications

!!! warning "FileSystemWatcher delays (folder-as-files only)"

The `FileSystemWatcher` used by `add_folder_as_files()` may have slight delays in detecting real-time changes to a mod author's mod folder.

**Typical delay:** <20ms on local filesystems

**Limitations:**

- ⚠️ Network filesystems may have longer delays
- ⚠️ Some filesystems don't support change notifications (old NFS, some remote mounts)
- ✅ High-frequency changes may be coalesced (desirable - reduces overhead)

**What this means:**

- `add_file()`: No watcher, changes to mod files not tracked automatically
- `add_folder_as_files()`: Watcher active, changes detected within <20ms
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

!!! note "Priority customization not needed"

    We could provide an API to change redirect priority, but that is not needed at the moment.

### Directory Moves During Runtime

!!! danger "Moving mod folders during runtime is unsupported"

    Moving or renaming the physical mod folder on disk while the game is running is unsupported, both in the VFS and in Reloaded3 itself.

If a mod folder's location on disk is moved or renamed while the game is running, behaviour depends on redirect type:

**File redirects (`add_file()`):**

- ❌ Break immediately (hardcoded path no longer valid)
- Open file handles remain valid but point to old location
- New file opens will fail (path doesn't exist)

**Folder-as-files (`add_folder_as_files()`):**

- `FileSystemWatcher` will detect the folder move and remove all file redirects
- Open file handles remain valid but point to old location

**Folder fallback (`add_folder()`):**

- ❌ Break immediately (hardcoded path no longer valid)
- Lookups will fall back to original location
- Open file handles remain valid but point to old location

**Recommendation:** Don't move or rename entire mod folders on disk while the game is running. Edits within the folder are fine.

## Workflows with Undefined Behaviour

### Modding Tools That Modify Game Folder

!!! warning "In rare cases, modding tools operate on a pre-modded game folder and need to create/modify/delete files"

**Examples:**

- **xEdit (Bethesda Games)**: Expects to create/modify plugin files in game's `Data` folder
- **Bethesda Creation Kit(s)**: Expects to write directly to game folder
- **Asset extractors** that write to game folder

**Solution: Use an overrides folder**

You can use the `add_folder()` API to create an 'overrides' folder where all writes are redirected. These writes will later need to be resolved by the user and mod manager or similar application.

!!! info "Similar to existing workflows"

    This is no different than managing 'External Changes' in Nexus Mods App - tools make changes, and the mod manager resolves them later.

#### Delete-and-Recreate Pattern

!!! info "Awareness required: Tools that delete and recreate files"

Some modding tools could potentially delete and then recreate a file:

1. Read original file (from redirected location if VFS active)
2. Delete original file (deletes from redirected location)
3. Write new file with same name (writes to redirected location due to folder redirect)

**What happens with VFS:** The modified file will be written to the redirected location (e.g., `overrides/` folder or a mod's folder), not the original game location. This is expected behaviour with folder redirects and sticky redirects.

**Where files end up:** The modified file lands in the 'overrides' or mod folder, not the original game directory. Mod managers should track these changes and let users resolve them - same as Nexus Mods App's 'External Changes' workflow.

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

