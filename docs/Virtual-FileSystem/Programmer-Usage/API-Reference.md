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

### Understanding File vs Folder Redirects

The VFS provides two types of redirection with different purposes:

**File Redirects** (Primary - Fast Path):

- Map individual files: `File A → File B`
- Checked first, highest priority
- Stored in optimized data structure for fast lookups
- **This is the default and recommended approach**

**Folder Redirects** (Fallback - Dynamic Content):

- Map folder paths recursively: `Foo/ → Kitty/`
- Only checked when no file redirect exists
- Used for save file redirection and dynamic content where files are not known in advance
- Can be nested: `Foo/Bar → Kitty/Kat` and `Foo/Bar/Baz → Nya/Nyan`
- Recursive: `Foo → Kitty` means `Foo/Bar/Baz/File.txt → Kitty/Bar/Baz/File.txt`

!!! warning "Folder redirects are fallback only"

    Folder redirects are a secondary mechanism used when no file redirect matches.
    
    **Use file redirects for:** Loading assets from elsewhere (textures, models, audio, etc.). File redirects are more performant.
    
    **Use folder redirects for:** Scenarios where you expect writes or when files may be created in a folder and we cannot know their names ahead of time (e.g., save files, user-generated content).

!!! tip "Priority order"

    1. **File redirects** are checked first
    2. **Folder redirects** are checked as fallback
    3. Within each category, later additions take precedence over earlier ones

### Redirecting Individual Files

Redirects an individual file from `source_path` (original game path) to `target_path` (mod file path).

!!! info "File redirects from folders"

    When adding file redirects from `source_folder` to `target_folder`, the VFS scans `target_folder` and creates individual file redirects for each file found.
    
    A `FileSystemWatcher` is automatically created to monitor `target_folder` for changes, updating redirects in real-time when files are added, removed, or modified.
    
    This provides the performance benefits of file-level redirects while maintaining dynamic behavior.

=== "Rust"
    ```rust
    fn add_redirect(&self, source_path: &str, target_path: &str) -> Result<RedirectHandle, VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_add(
        const char* source_path,
        const char* target_path,
        RedirectHandle* handle_out
    );
    ```

=== "C++"
    ```cpp
    RedirectHandle addRedirect(std::string_view source_path, std::string_view target_path);
    ```

=== "C#"
    ```csharp
    RedirectHandle AddRedirect(string sourcePath, string targetPath);
    ```

**Removing redirects:**

=== "Rust"
    ```rust
    fn remove_redirect(&self, handle: RedirectHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_remove(RedirectHandle handle);
    ```

=== "C++"
    ```cpp
    void removeRedirect(RedirectHandle handle);
    ```

=== "C#"
    ```csharp
    void RemoveRedirect(RedirectHandle handle);
    ```

### Redirecting Folders (Fallback)

Adds a fallback folder redirect. Files in `target_folder` will be accessible at `source_folder`.

!!! warning "Use sparingly"

    Folder redirects are less efficient than file redirects. Use them only for scenarios where files may be created in a folder and we cannot know their names ahead of time (e.g., save files, user-generated content).

=== "Rust"
    ```rust
    fn add_folder_redirect(&self, source_folder: &str, target_folder: &str) -> Result<FolderRedirectHandle, VfsError>
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
    FolderRedirectHandle addFolderRedirect(std::string_view source_folder, std::string_view target_folder);
    ```

=== "C#"
    ```csharp
    FolderRedirectHandle AddFolderRedirect(string sourceFolder, string targetFolder);
    ```

**Removing folder redirects:**

=== "Rust"
    ```rust
    fn remove_folder_redirect(&self, handle: FolderRedirectHandle) -> Result<(), VfsError>
    ```

=== "C Export"
    ```c
    R3VfsResult r3vfs_redirector_remove_folder(FolderRedirectHandle handle);
    ```

=== "C++"
    ```cpp
    void removeFolderRedirect(FolderRedirectHandle handle);
    ```

=== "C#"
    ```csharp
    void RemoveFolderRedirect(FolderRedirectHandle handle);
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
typedef struct R3VfsRedirect* RedirectHandle;
typedef struct R3VfsFolderRedirect* FolderRedirectHandle;
typedef struct R3VfsVirtualFile* VirtualFileHandle;

// Invalid handle constants
#define R3VFS_INVALID_REDIRECT_HANDLE NULL
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
    let handle = redirector.add_redirect(
        r"dvdroot\bgm\SNG_STG26.adx", 
        r"mods\mybgm.adx"
    )?;
    // ...
    redirector.remove_redirect(handle)?;
    ```

=== "C"
    ```c
    RedirectHandle handle;
    R3VfsResult result = r3vfs_redirector_add(
        "dvdroot/bgm/SNG_STG26.adx",
        "mods/mybgm.adx",
        &handle
    );
    if (result != R3VFS_OK) {
        // Handle error
    }
    // ...
    r3vfs_redirector_remove(handle);
    ```

=== "C++"
    ```cpp
    auto handle = _redirector->AddRedirect(
        R"(dvdroot\bgm\SNG_STG26.adx)", 
        R"(mods\mybgm.adx)"
    );
    // ...
    _redirector->RemoveRedirect(handle);
    ```

=== "C#"
    ```csharp
    var handle = _redirector.AddRedirect(@"dvdroot\bgm\SNG_STG26.adx", @"mods\mybgm.adx");
    // ...
    _redirector.RemoveRedirect(handle);
    ```

### Folder Redirect (for Save Files)

=== "Rust"
    ```rust
    // Redirect save files to a different location
    let handle = redirector.add_folder_redirect(
        r"game\saves", 
        r"mods\mymod\saves"
    )?;
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Even for files created at runtime
    ```

=== "C"
    ```c
    // Redirect save files to a different location
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
    // Even for files created at runtime
    ```

=== "C++"
    ```cpp
    // Redirect save files to a different location
    auto handle = _redirector->AddFolderRedirect(
        R"(game\saves)", 
        R"(mods\mymod\saves)"
    );
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Even for files created at runtime
    ```

=== "C#"
    ```csharp
    // Redirect save files to a different location
    var handle = _redirector.AddFolderRedirect(@"game\saves", @"mods\mymod\saves");
    // Now: game\saves\profile1.sav -> mods\mymod\saves\profile1.sav
    // Even for files created at runtime
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
    // Add many redirects
    for i in 0..1000 {
        redirector.add_redirect(
            &format!("game/file{}.dat", i),
            &format!("mods/file{}.dat", i)
        )?;
    }

    // Trigger optimization
    redirector.optimize()?;
    ```

=== "C"
    ```c
    // Add many redirects
    for (int i = 0; i < 1000; i++) {
        char source[256], target[256];
        snprintf(source, sizeof(source), "game/file%d.dat", i);
        snprintf(target, sizeof(target), "mods/file%d.dat", i);
        
        RedirectHandle handle;
        r3vfs_redirector_add(source, target, &handle);
    }

    // Trigger optimization
    r3vfs_redirector_optimize();
    ```

=== "C++"
    ```cpp
    // Add many redirects
    for (int i = 0; i < 1000; i++) {
        auto source = std::format("game/file{}.dat", i);
        auto target = std::format("mods/file{}.dat", i);
        _redirector->AddRedirect(source, target);
    }

    // Trigger optimization
    _redirector->Optimize();
    ```

=== "C#"
    ```csharp
    // Add many redirects
    for (int i = 0; i < 1000; i++) {
        _redirector.AddRedirect($"game/file{i}.dat", $"mods/file{i}.dat");
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

The following APIs may be implemented in the future as needed:

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
uint32_t r3vfs_redirector_get_count(void);
uint32_t r3vfs_redirector_get_folder_count(void);
uint32_t r3vfs_vfile_get_count(void);

// Enumerate redirects (callback-based)
typedef void (*R3VfsEnumerateCallback)(
    const char* source,
    const char* target,
    void* user_data
);

void r3vfs_redirector_enumerate(R3VfsEnumerateCallback callback, void* user_data);
void r3vfs_redirector_enumerate_folders(R3VfsEnumerateCallback callback, void* user_data);
```

### Statistics & Performance Monitoring

!!! info "Requires feature flag"

    These APIs would only be available when compiled with the `vfs_statistics` feature flag.

```c
#[cfg(feature = "vfs_statistics")]
typedef struct {
    uint64_t total_redirects_hit;         // Number of file redirects used
    uint64_t total_folder_redirects_hit;  // Number of folder redirects used
    uint64_t total_virtual_files_hit;     // Number of virtual file accesses
    uint64_t total_cache_hits;            // Internal cache performance
    uint64_t total_cache_misses;
} R3VfsStatistics;

#[cfg(feature = "vfs_statistics")]
R3VfsResult r3vfs_statistics_get(R3VfsStatistics* stats_out);

#[cfg(feature = "vfs_statistics")]
void r3vfs_statistics_reset(void);
```
