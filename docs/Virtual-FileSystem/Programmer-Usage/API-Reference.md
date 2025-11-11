# API Reference

!!! warning "This is a plan/prototype - finer details may change during development"

    All APIs shown here are for reference only and may be refined during implementation.

## Error Handling

All C API functions return a `R3VfsResult` to indicate success or failure:

```c
typedef enum {
    R3VFS_OK = 0,
    R3VFS_ERROR_NOT_INITIALIZED = -1,
    R3VFS_ERROR_INVALID_PATH = -2,
    R3VFS_ERROR_PATH_TOO_LONG = -3,
    R3VFS_ERROR_OUT_OF_MEMORY = -4,
    R3VFS_ERROR_ALREADY_EXISTS = -5,
    R3VFS_ERROR_NOT_FOUND = -6,
    R3VFS_ERROR_INVALID_HANDLE = -7,
} R3VfsResult;
```

!!! tip "Direct error returns"

    Errors are returned directly from functions - there is no `GetLastError()` or `errno` pattern.

---

## Redirector

This API handles file and folder path redirection.

!!! info "Language-specific naming"

    - **Rust**: `Redirector`
    - **C**: `r3vfs_redirector_*` functions
    - **C++**: `Redirector`
    - **C#**: `IRedirector`

### Understanding Redirect Types

The VFS uses **two lookup tiers**:

**Tier 1: File Redirects** (Fast - Checked First):

- Map individual files: `File A → File B`
- Checked first, highest priority

**Tier 2: Folder Redirects** (Fallback - Checked Second):

- Map folder paths recursively: `Foo/ → Kitty/`
- Only checked when no file redirect exists
- **Use when you expect writes to non-mod (game) folders** (e.g., save files, config files)
- Recursive: `Foo → Kitty` means `Foo/Bar/Baz/File.txt → Kitty/Bar/Baz/File.txt`
- Can point subfolders to different paths, e.g. `Foo/Bar → Kitty/Kat` and `Foo/Bar/Baz → Nya/Nyan`
- **Slower than file redirects**

!!! tip "Lookup priority"

    1. **File redirects** (Tier 1) - checked first
    2. **Folder redirects** (Tier 2) - checked only if no file redirect found
    3. Within the same tier, later additions take precedence over earlier ones

### Redirecting Individual Files

Redirects an individual file from `source_path` (original game path) to `target_path` (mod file path).

=== "Rust"
    ```rust
    fn add_file(&self, source_path: &str, target_path: &str) -> Result<RedirectHandle, VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_add_file(
        const char* source_path,
        const char* target_path,
        RedirectHandle* handle_out
    );
    ```

=== "C++"
    ```cpp
    RedirectHandle addFile(std::string_view source_path, std::string_view target_path);
    ```

=== "C#"
    ```csharp
    RedirectHandle AddFile(string sourcePath, string targetPath);
    ```

**Removing file redirects:**

=== "Rust"
    ```rust
    fn remove_file(&self, handle: RedirectHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_remove_file(RedirectHandle handle);
    ```

=== "C++"
    ```cpp
    void removeFile(RedirectHandle handle);
    ```

=== "C#"
    ```csharp
    void RemoveFile(RedirectHandle handle);
    ```

### Redirecting Folders as Files

Tracks the current state of `target_folder` and automatically creates individual file redirects for each file found. A `FileSystemWatcher` monitors `target_folder` for changes, automatically adding/removing file redirects when files are created, deleted, or modified.

Used to map mod folders to game folders. Supports real-time edits of content in mod folder.

!!! tip "Recommended for most mod scenarios"

    This provides the performance benefits of file redirects (Tier 1 lookup) with the convenience of folder-based management. The `FileSystemWatcher` ensures redirects stay synchronized with the filesystem.

=== "Rust"
    ```rust
    fn add_folder_as_files(&self, source_folder: &str, target_folder: &str) -> Result<FolderFilesHandle, VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_add_folder_as_files(
        const char* source_folder,
        const char* target_folder,
        FolderFilesHandle* handle_out
    );
    ```

=== "C++"
    ```cpp
    FolderFilesHandle addFolderAsFiles(std::string_view source_folder, std::string_view target_folder);
    ```

=== "C#"
    ```csharp
    FolderFilesHandle AddFolderAsFiles(string sourceFolder, string targetFolder);
    ```

**Removing folder-as-files redirects:**

=== "Rust"
    ```rust
    fn remove_folder_as_files(&self, handle: FolderFilesHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_remove_folder_as_files(FolderFilesHandle handle);
    ```

=== "C++"
    ```cpp
    void removeFolderAsFiles(FolderFilesHandle handle);
    ```

=== "C#"
    ```csharp
    void RemoveFolderAsFiles(FolderFilesHandle handle);
    ```

### Redirecting Folders (Fallback)

Adds a Tier 2 folder fallback redirect. Files in `target_folder` will be accessible at `source_folder` only when no file redirect matches.

!!! warning "Use when you expect writes to non-mod (game) folders"

    Folder redirects are less efficient than file redirects (Tier 1). Use them when you expect the **game** to write files (e.g., save files, config files, user-generated content).
    
    **If you expect writes to mod folders**, use `add_folder_as_files()` instead - the `FileSystemWatcher` will automatically track changes and provide better performance.

=== "Rust"
    ```rust
    fn add_folder(&self, source_folder: &str, target_folder: &str) -> Result<FolderRedirectHandle, VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_add_folder(
        const char* source_folder,
        const char* target_folder,
        FolderRedirectHandle* handle_out
    );
    ```

=== "C++"
    ```cpp
    FolderRedirectHandle addFolder(std::string_view source_folder, std::string_view target_folder);
    ```

=== "C#"
    ```csharp
    FolderRedirectHandle AddFolder(string sourceFolder, string targetFolder);
    ```

**Removing folder redirects:**

=== "Rust"
    ```rust
    fn remove_folder(&self, handle: FolderRedirectHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_remove_folder(FolderRedirectHandle handle);
    ```

=== "C++"
    ```cpp
    void removeFolder(FolderRedirectHandle handle);
    ```

=== "C#"
    ```csharp
    void RemoveFolder(FolderRedirectHandle handle);
    ```

### Optimization

Triggers optimization to build the efficient RedirectionTree structure for faster lookups.

!!! tip "When to call optimize"

    Call `optimize()` after adding all initial redirects. This builds the fast RedirectionTree structure.
    
    See [../Performance.md](../Performance.md) for details.

=== "Rust"
    ```rust
    fn optimize(&self) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_optimize(void);
    ```

=== "C++"
    ```cpp
    void optimize();
    ```

=== "C#"
    ```csharp
    void Optimize();
    ```

---

## VirtualFiles

This API handles virtual file registration (Layer 2).

!!! info "Private API for Layer 2"

    This API is intended to be called by the File Emulation Framework (Layer 2), not by end users or individual file emulators.

!!! info "Language-specific naming"

    - **Rust**: `VirtualFiles`
    - **C**: `r3vfs_vfile_*` functions
    - **C++**: `VirtualFiles`
    - **C#**: `IVirtualFiles`

### Registering Virtual Files

!!! warning "Layer 2 API - Direct usage discouraged"

    This is a **Layer 2 internal API**. It should only be called by the File Emulation Framework (Layer 2), not by end users or mods.
    
    **For mod authors**: Use Layer 2's public `RegisterVirtualFile()` API instead, which accepts a `fileHandler` parameter. See the [main documentation](../../index.md#layer-2-virtual-file-framework) for details.
    
    Direct usage of this Layer 1 API bypasses the file content handling Layer 2 provides.

Registers a new virtual file at `file_path` with the provided metadata. This allows the virtual file to appear in directory searches and be opened by applications.

=== "Rust"
    ```rust
    fn register_virtual_file(&self, file_path: &str, metadata: VirtualFileMetadata) -> Result<VirtualFileHandle, VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_vfile_register(
        const char* file_path,
        const VirtualFileMetadata* metadata,
        VirtualFileHandle* handle_out
    );
    ```

=== "C++"
    ```cpp
    VirtualFileHandle registerVirtualFile(std::string_view file_path, const VirtualFileMetadata& metadata);
    ```

=== "C#"
    ```csharp
    VirtualFileHandle RegisterVirtualFile(string filePath, VirtualFileMetadata metadata);
    ```

**Unregistering virtual files:**

=== "Rust"
    ```rust
    fn unregister_virtual_file(&self, handle: VirtualFileHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_vfile_unregister(VirtualFileHandle handle);
    ```

=== "C++"
    ```cpp
    void unregisterVirtualFile(VirtualFileHandle handle);
    ```

=== "C#"
    ```csharp
    void UnregisterVirtualFile(VirtualFileHandle handle);
    ```

### VirtualFileMetadata Structure

```rust
#[repr(C)]
pub struct VirtualFileMetadata {
    pub creation_time: i64,
    pub last_access_time: i64,
    pub last_write_time: i64,
    pub change_time: i64,
    pub end_of_file: i64,        // File size in bytes
    pub allocation_size: i64,    // Allocated size (usually rounded to block size)
    pub file_attributes: FileAttributes,
}
```

!!! note "Metadata is immutable"

    Metadata can only be set once when creating the virtual file and cannot be modified afterward.
    
    This may change with read/write support in the future, where handle-based metadata updates could be supported.

**FileAttributes** (platform-specific):

```rust
#[cfg(windows)]
pub type FileAttributes = u32; // Win32 FILE_ATTRIBUTE_* flags

#[cfg(unix)]
pub type FileAttributes = u32; // mode_t equivalent

// Common Windows constants
pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
pub const FILE_ATTRIBUTE_READONLY: u32 = 0x01;
pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
pub const FILE_ATTRIBUTE_HIDDEN: u32 = 0x02;
pub const FILE_ATTRIBUTE_SYSTEM: u32 = 0x04;
pub const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;
```

---

## Settings

This API handles VFS configuration and toggles.

!!! info "Language-specific naming"

    - **Rust**: `Settings`
    - **C**: `r3vfs_settings_*` functions
    - **C++**: `Settings`
    - **C#**: `ISettings`

### Settings

Gets or sets individual VFS settings.

!!! note "Debug logging only"

    Logging is only available in debug/logging-enabled builds. Release builds compile out all logging code for performance.

=== "Rust"
    ```rust
    fn get_setting(&self, setting: VfsSetting) -> bool
    fn set_setting(&self, setting: VfsSetting, enable: bool)
    
    pub enum VfsSetting {
        PrintRedirect,      // Print when a file redirect is performed
        PrintOpen,          // Print file open operations (debug)
        DontPrintNonFiles,  // Skip printing non-files to console
        PrintGetAttributes, // Print attribute query operations (debug)
    }
    ```

=== "C Export"
    ```c
    typedef enum {
        R3VFS_SETTING_PRINT_REDIRECT = 0,
        R3VFS_SETTING_PRINT_OPEN = 1,
        R3VFS_SETTING_DONT_PRINT_NON_FILES = 2,
        R3VFS_SETTING_PRINT_GET_ATTRIBUTES = 3,
    } R3VfsSetting;
    
    bool r3vfs_settings_get(R3VfsSetting setting);
    void r3vfs_settings_set(R3VfsSetting setting, bool enable);
    ```

=== "C++"
    ```cpp
    enum class VfsSetting {
        PrintRedirect = 0,
        PrintOpen = 1,
        DontPrintNonFiles = 2,
        PrintGetAttributes = 3,
    };
    
    bool getSetting(VfsSetting setting);
    void setSetting(VfsSetting setting, bool enable);
    ```

=== "C#"
    ```csharp
    enum VfsSetting {
        PrintRedirect = 0,
        PrintOpen = 1,
        DontPrintNonFiles = 2,
        PrintGetAttributes = 3,
    }
    
    bool GetSetting(VfsSetting setting);
    void SetSetting(VfsSetting setting, bool enable);
    ```

### Enable/Disable VFS

Enables or disables the VFS entirely.

=== "Rust"
    ```rust
    fn enable(&self)
    fn disable(&self)
    ```

=== "C Export"
    ```c
    void r3vfs_settings_enable(void);
    void r3vfs_settings_disable(void);
    ```

=== "C++"
    ```cpp
    void enable();
    void disable();
    ```

=== "C#"
    ```csharp
    void Enable();
    void Disable();
    ```

---

## Handle Types

```c
// Opaque handles (pointers to internal structures)
typedef struct R3VfsRedirect* RedirectHandle;              // Tier 1: Individual file redirects
typedef struct R3VfsFolderFiles* FolderFilesHandle;        // Tier 2: Folder scanned as files
typedef struct R3VfsFolderRedirect* FolderRedirectHandle;  // Tier 3: Folder fallback redirects
typedef struct R3VfsVirtualFile* VirtualFileHandle;        // Virtual file registration

// Invalid handle constants
#define R3VFS_INVALID_REDIRECT_HANDLE NULL
#define R3VFS_INVALID_FOLDER_FILES_HANDLE NULL
#define R3VFS_INVALID_FOLDER_REDIRECT_HANDLE NULL
#define R3VFS_INVALID_VIRTUAL_FILE_HANDLE NULL
```

---

## Thread Safety

!!! warning "Thread Safety Contract"

    **APIs are thread-safe**: Multiple threads can call VFS APIs concurrently. The VFS uses internal synchronization to protect shared state.
    
    **Handles are tied to state**: Each handle (RedirectHandle, FolderRedirectHandle, VirtualFileHandle) references internal state. Using the same handle from multiple threads concurrently is a race condition. Use external synchronization if you need to share handles across threads.

---

## Examples

### File Redirect

=== "Rust"
    ```rust
    let handle = redirector.add_file(
        r"dvdroot\bgm\SNG_STG26.adx", 
        r"mods\mybgm.adx"
    )?;
    // ...
    redirector.remove_file(handle)?;
    ```

=== "C"
    ```c
    RedirectHandle handle;
    R3VfsResult result = r3vfs_redirector_add_file(
        "dvdroot/bgm/SNG_STG26.adx",
        "mods/mybgm.adx",
        &handle
    );
    if (result != R3VFS_OK) {
        // Handle error
    }
    // ...
    r3vfs_redirector_remove_file(handle);
    ```

=== "C++"
    ```cpp
    auto handle = _redirector->AddFile(
        R"(dvdroot\bgm\SNG_STG26.adx)", 
        R"(mods\mybgm.adx)"
    );
    // ...
    _redirector->RemoveFile(handle);
    ```

=== "C#"
    ```csharp
    var handle = _redirector.AddFile(@"dvdroot\bgm\SNG_STG26.adx", @"mods\mybgm.adx");
    // ...
    _redirector.RemoveFile(handle);
    ```

### Folder-as-Files (Automatic File Redirects)

=== "Rust"
    ```rust
    // Scan mod folder and create file redirects with FileSystemWatcher
    let handle = redirector.add_folder_as_files(
        r"dvdroot\textures", 
        r"mods\mytextures"
    )?;
    // VFS scans mods\mytextures and creates file redirects for each file
    // FileSystemWatcher monitors for changes
    // ...
    redirector.remove_folder_as_files(handle)?;
    ```

=== "C"
    ```c
    // Scan mod folder and create file redirects with FileSystemWatcher
    FolderFilesHandle handle;
    R3VfsResult result = r3vfs_redirector_add_folder_as_files(
        "dvdroot/textures",
        "mods/mytextures",
        &handle
    );
    if (result != R3VFS_OK) {
        // Handle error
    }
    // VFS scans mods/mytextures and creates file redirects for each file
    // FileSystemWatcher monitors for changes
    // ...
    r3vfs_redirector_remove_folder_as_files(handle);
    ```

=== "C++"
    ```cpp
    // Scan mod folder and create file redirects with FileSystemWatcher
    auto handle = _redirector->AddFolderAsFiles(
        R"(dvdroot\textures)", 
        R"(mods\mytextures)"
    );
    // VFS scans mods\mytextures and creates file redirects for each file
    // FileSystemWatcher monitors for changes
    // ...
    _redirector->RemoveFolderAsFiles(handle);
    ```

=== "C#"
    ```csharp
    // Scan mod folder and create file redirects with FileSystemWatcher
    var handle = _redirector.AddFolderAsFiles(@"dvdroot\textures", @"mods\mytextures");
    // VFS scans mods\mytextures and creates file redirects for each file
    // FileSystemWatcher monitors for changes
    // ...
    _redirector.RemoveFolderAsFiles(handle);
    ```

### Folder Redirect (Fallback for Save Files)

=== "Rust"
    ```rust
    // Redirect save files to a different location (fallback lookup)
    let handle = redirector.add_folder(
        r"game\saves", 
        r"mods\mymod\saves"
    )?;
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Works even for files created at runtime (unknown filenames)
    ```

=== "C"
    ```c
    // Redirect save files to a different location (fallback lookup)
    FolderRedirectHandle handle;
    R3VfsResult result = r3vfs_redirector_add_folder(
        "game/saves",
        "mods/mymod/saves",
        &handle
    );
    if (result != R3VFS_OK) {
        // Handle error
    }
    // Now: game/saves/profile1.sav -> mods/mymod/saves/profile1.sav
    // Works even for files created at runtime (unknown filenames)
    ```

=== "C++"
    ```cpp
    // Redirect save files to a different location (fallback lookup)
    auto handle = _redirector->AddFolder(
        R"(game\saves)", 
        R"(mods\mymod\saves)"
    );
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Works even for files created at runtime (unknown filenames)
    ```

=== "C#"
    ```csharp
    // Redirect save files to a different location (fallback lookup)
    var handle = _redirector.AddFolder(@"game\saves", @"mods\mymod\saves");
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Works even for files created at runtime (unknown filenames)
    ```

### Register Virtual File

=== "Rust"
    ```rust
    use std::time::{SystemTime, UNIX_EPOCH};

    let metadata = VirtualFileMetadata {
        creation_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
        last_access_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
        last_write_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
        change_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64,
        end_of_file: 1024,
        allocation_size: 1024,
        file_attributes: FILE_ATTRIBUTE_NORMAL,
    };

    let handle = virtual_files.register_virtual_file(r"game\virtualfile.txt", metadata)?;
    // ...
    virtual_files.unregister_virtual_file(handle)?;
    ```

=== "C"
    ```c
    VirtualFileMetadata metadata = {
        .creation_time = get_current_time(),
        .last_access_time = get_current_time(),
        .last_write_time = get_current_time(),
        .change_time = get_current_time(),
        .end_of_file = 1024,
        .allocation_size = 1024,
        .file_attributes = FILE_ATTRIBUTE_NORMAL
    };

    VirtualFileHandle handle;
    R3VfsResult result = r3vfs_vfile_register(
        "game/virtualfile.txt",
        &metadata,
        &handle
    );
    if (result != R3VFS_OK) {
        // Handle error
    }
    // ...
    r3vfs_vfile_unregister(handle);
    ```

=== "C++"
    ```cpp
    VirtualFileMetadata metadata;
    metadata.creation_time = std::chrono::system_clock::now().time_since_epoch().count();
    metadata.last_access_time = std::chrono::system_clock::now().time_since_epoch().count();
    metadata.last_write_time = std::chrono::system_clock::now().time_since_epoch().count();
    metadata.change_time = std::chrono::system_clock::now().time_since_epoch().count();
    metadata.end_of_file = 1024;
    metadata.allocation_size = 1024;
    metadata.file_attributes = FILE_ATTRIBUTE_NORMAL;

    auto handle = _virtualFiles->RegisterVirtualFile(R"(game\virtualfile.txt)", metadata);
    // ...
    _virtualFiles->UnregisterVirtualFile(handle);
    ```

=== "C#"
    ```csharp
    var metadata = new VirtualFileMetadata
    {
        CreationTime = DateTime.Now.Ticks,
        LastAccessTime = DateTime.Now.Ticks,
        LastWriteTime = DateTime.Now.Ticks,
        ChangeTime = DateTime.Now.Ticks,
        EndOfFile = 1024,
        AllocationSize = 1024,
        FileAttributes = FileAttributes.Normal
    };

    var handle = _virtualFiles.RegisterVirtualFile(@"game\virtualfile.txt", metadata);
    // ...
    _virtualFiles.UnregisterVirtualFile(handle);
    ```

### Change Settings

=== "Rust"
    ```rust
    // Enable printing of file redirects
    settings.set_setting(VfsSetting::PrintRedirect, true);

    // Disable the VFS entirely
    settings.disable();
    ```

=== "C"
    ```c
    // Enable printing of file redirects
    r3vfs_settings_set(R3VFS_SETTING_PRINT_REDIRECT, true);

    // Disable the VFS entirely
    r3vfs_settings_disable();
    ```

=== "C++"
    ```cpp
    // Enable printing of file redirects
    _settings->SetSetting(VfsSetting::PrintRedirect, true);

    // Disable the VFS entirely
    _settings->Disable();
    ```

=== "C#"
    ```csharp
    // Enable printing of file redirects
    _settings.SetSetting(VfsSetting.PrintRedirect, true);

    // Disable the VFS entirely
    _settings.Disable();
    ```

### Optimize After Adding Redirects

=== "Rust"
    ```rust
    // Add many file redirects
    for i in 0..1000 {
        redirector.add_file(
            &format!("game/file{}.dat", i),
            &format!("mods/file{}.dat", i)
        )?;
    }

    // Trigger optimization
    redirector.optimize()?;
    ```

=== "C"
    ```c
    // Add many file redirects
    for (int i = 0; i < 1000; i++) {
        char source[256], target[256];
        snprintf(source, sizeof(source), "game/file%d.dat", i);
        snprintf(target, sizeof(target), "mods/file%d.dat", i);
        
        RedirectHandle handle;
        r3vfs_redirector_add_file(source, target, &handle);
    }

    // Trigger optimization
    r3vfs_redirector_optimize();
    ```

=== "C++"
    ```cpp
    // Add many file redirects
    for (int i = 0; i < 1000; i++) {
        auto source = std::format("game/file{}.dat", i);
        auto target = std::format("mods/file{}.dat", i);
        _redirector->AddFile(source, target);
    }

    // Trigger optimization
    _redirector->Optimize();
    ```

=== "C#"
    ```csharp
    // Add many file redirects
    for (int i = 0; i < 1000; i++) {
        _redirector.AddFile($"game/file{i}.dat", $"mods/file{i}.dat");
    }

    // Trigger optimization
    _redirector.Optimize();
    ```

---

## Implementation

The VFS is written in Rust with C exports. C headers are automatically generated using [cbindgen](https://github.com/mozilla/cbindgen) and C# bindings using [csbindgen](https://github.com/Cysharp/csbindgen).

!!! info "Higher-level safe wrappers"

    Safe higher-level bindings for languages like C++, C#, and others will be up to the community to create, or will be provided when there is considerable demand.
    
    The C exports provide a stable foundation for building idiomatic wrappers in any language.

---

## Nice-to-Have APIs (Future)

!!! info "Future APIs"

    The following APIs may be implemented in the future as needed.

### Query & Introspection

```c
// Check if a path is redirected
bool r3vfs_redirector_is_path_redirected(const char* source_path);

// Get redirect target for a path
R3VfsResult r3vfs_redirector_get_target(
    const char* source_path,
    char* target_out,
    size_t target_len
);

// Check if a path is a virtual file
bool r3vfs_vfile_is_virtual(const char* path);

// Get counts (for debugging/stats)
uint32_t r3vfs_redirector_get_file_count(void);              // Count of individual file redirects
uint32_t r3vfs_redirector_get_folder_files_count(void);      // Count of folder-as-files redirects
uint32_t r3vfs_redirector_get_folder_count(void);            // Count of folder fallback redirects
uint32_t r3vfs_vfile_get_count(void);                        // Count of virtual files

// Enumerate redirects (callback-based)
typedef void (*R3VfsEnumerateCallback)(
    const char* source,
    const char* target,
    void* user_data
);

void r3vfs_redirector_enumerate_files(R3VfsEnumerateCallback callback, void* user_data);
void r3vfs_redirector_enumerate_folder_files(R3VfsEnumerateCallback callback, void* user_data);
void r3vfs_redirector_enumerate_folders(R3VfsEnumerateCallback callback, void* user_data);
```

### Statistics & Performance Monitoring

!!! info "Requires feature flag"

    These APIs would only be available when compiled with the `vfs_statistics` feature flag.

```c
#ifdef R3VFS_FEATURE_STATISTICS
typedef struct {
    uint64_t total_redirects_hit;         // Number of file redirects used
    uint64_t total_folder_redirects_hit;  // Number of folder redirects used
    uint64_t total_virtual_files_hit;     // Number of virtual file accesses
    uint64_t total_cache_hits;            // Internal cache performance
    uint64_t total_cache_misses;
} R3VfsStatistics;

R3VfsResult r3vfs_statistics_get(R3VfsStatistics* stats_out);
void r3vfs_statistics_reset(void);
#endif // R3VFS_FEATURE_STATISTICS
```
