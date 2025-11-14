# DirectStorage & IoRing (Layer 2)

!!! info "Layer 2 Only"
    
    **This is only needed for Layer 2 (Virtual Files).** For File Redirection (Layer 1), no action is needed.

Microsoft DirectStorage is a ***read only*** [asynchronous I/O API][directstorage-dataflow] aimed at bringing out the best in NVMe SSDs, particularly when it comes to reducing the amount of overhead for small data reads.

This page explains how VFS interacts with DirectStorage.

!!! warning "Common Misconception"
    
    ***Unlike popular belief***, [DirectStorage on Windows does **not** transfer data directly from file to GPU bypassing CPU memory](https://github.com/microsoft/DirectStorage/blob/main/Docs/DeveloperGuidance.md#uncompressed-data-flow).
    
    Data flows through intermediate buffers on the CPU/RAM.

## DirectStorage API Usage Breakdown

!!! info "DirectStorage uses different I/O mechanisms depending on the Windows version"

    - **Windows 10**: Uses `ReadFile` to read from files
    - **Windows 11**: Uses [IoRing][ioring-docs], a high-performance I/O submission mechanism inspired by Linux's `io_uring`

!!! note "Wine on Linux (today/10.x) uses the Windows 10 path"
    
    It doesn't currently implement Windows' IoRing APIs, so DirectStorage in Wine falls back to the Windows 10 `ReadFile` path.

To be more specific, DirectStorage uses the following APIs for file operations:

| Operation                         | API                                    | Notes                                          |
| --------------------------------- | -------------------------------------- | ---------------------------------------------- |
| **Open File**                     | `CreateFileW` (Win32)                  | Used across all platforms                      |
| **Get File Information**          | `GetFileInformationByHandle` (Win32)   | Available since Windows XP                     |
| **Read File (Windows 10 & Wine)** | `ReadFile` (Win32) w/ `'Overlapped'`   | Traditional async I/O, calls into `NtReadFile` |
| **Read File (Windows 11)**        | `BuildIoRingReadFile` & `SubmitIoRing` | IoRing-based I/O (new to Win11)                |

!!! info "Implementation Details"
    
    Under the hood, DirectStorage uses abstract C++ classes (interfaces) to implement platform-specific I/O:
    
    - **`IORingFileSystem`** - Windows 11 implementation using IoRing
    - **`Win32FileSystem`** - Windows 10 implementation using classic `Win32` API
    
    (ngl, they should have coded against NtDll directly rather than Win32 to shave some extra small # of instructions)

## What's an IoRing Anyway?

!!! question "So Windows 11 Uses This New IoRing Thing Anyway?"

IoRing is a form of asynchronous I/O based on Linux's [`io_uring`](https://www.youtube.com/watch?v=AaaH6skUEI8).

Instead of waiting for each read to complete, you tell the OS:

- Here's the buffer
- Here's the file handle
- Go read it from disk
- I'm going to do other things
- I'll come back later to check if it worked

You can queue up many operations at once, and the OS processes them in the background whilst your application continues running.

That's the simple, ELI5 version.

One of the key differences is:

- Normally when you read a file you do a copy `nvme -> kernel -> user`. (2 copies)
- With IoRing, the user and kernel buffer is shared so you do `nvme -> user` directly. (1 copy)
- This is the same as memory map, but without overhead of page faults.

!!! note "I need to add this to Nx2.0/R3A"

    I've never known Windows supports IoRing until now.
    This would be very useful for improving loads.<br/><br/>
    Bless Tim for giving me the opportunity to peek into DStorage on company time.

## DirectStorage I/O Paths

DirectStorage's implementation is abstracted behind C++ interfaces (`IORingFileSystem`, `Win32FileSystem`), making it difficult to trace the actual I/O flow. Here are working examples showing what the traditional and IoRing I/O stacks look like in practice:

=== "Windows 11 (IoRing)"

    ```c
    #include <windows.h>
    #include <ioringapi.h>

    // Create an I/O ring
    HIORING ioRing;
    IORING_CREATE_FLAGS flags = {0};
    flags.Required = IORING_CREATE_REQUIRED_FLAGS_NONE;

    HRESULT hr = CreateIoRing(
        IORING_VERSION_3,
        flags,
        256,  // submission queue size
        256,  // completion queue size
        &ioRing
    );

    // Open a file
    HANDLE hFile = CreateFileW(
        L"test.txt",
        GENERIC_READ,
        FILE_SHARE_READ,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        NULL
    );

    // Allocate buffer (In DirectStorage, this buffer will later be copied to GPU)
    const UINT32 bufferSize = 4096;
    BYTE* buffer = (BYTE*)VirtualAlloc(NULL, bufferSize, MEM_COMMIT, PAGE_READWRITE);

    // Register file and buffer
    IORING_HANDLE_REF fileRef = IoRingHandleRefFromHandle(hFile);
    IORING_BUFFER_REF bufferRef = IoRingBufferRefFromPointer(buffer);

    // Build the read operation
    hr = BuildIoRingReadFile(
        ioRing,
        fileRef,
        bufferRef,
        bufferSize,
        0,           // read from offset 0
        (UINT_PTR)1, // user data
        IOSQE_FLAGS_NONE
    );

    // Submit the operation
    hr = SubmitIoRing(ioRing, 1, 1000, NULL);

    // Get completion (DirectStorage will occasionally poll for completion)
    IORING_CQE cqe;
    hr = PopIoRingCompletion(ioRing, &cqe);

    // Check result
    if (SUCCEEDED(cqe.ResultCode)) {
        printf("Read %d bytes\n", cqe.Information);
    }

    // Cleanup
    CloseIoRing(ioRing);
    CloseHandle(hFile);
    VirtualFree(buffer, 0, MEM_RELEASE);
    ```

=== "Windows 10 (ReadFile)"

    ```c
    #include <windows.h>

    // Open a file
    HANDLE hFile = CreateFileW(
        L"test.txt",
        GENERIC_READ,
        FILE_SHARE_READ,
        NULL,
        OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED,
        NULL
    );

    // Allocate buffer (In DirectStorage, this buffer will later be copied to GPU)
    const DWORD bufferSize = 4096;
    BYTE* buffer = (BYTE*)VirtualAlloc(NULL, bufferSize, MEM_COMMIT, PAGE_READWRITE);

    // Read from file
    OVERLAPPED overlapped = {0};
    DWORD bytesRead = 0;
    
    if (ReadFile(hFile, buffer, bufferSize, &bytesRead, &overlapped)) {
        printf("Read %d bytes\n", bytesRead);
    } else if (GetLastError() == ERROR_IO_PENDING) {
        // Wait for completion (DirectStorage will occasionally check for completion)
        GetOverlappedResult(hFile, &overlapped, &bytesRead, TRUE);
        printf("Read %d bytes\n", bytesRead);
    }

    // Cleanup
    CloseHandle(hFile);
    VirtualFree(buffer, 0, MEM_RELEASE);
    ```

!!! note "IoRing's Builder Pattern"
    
    IoRing uses a builder-like pattern for composing I/O operations:
    
    1. **Build** operations with `BuildIoRingReadFile`, `BuildIoRingWriteFile`, etc. - these add operations to the submission queue
    2. **Submit** the queued operations with `SubmitIoRing` - this submits them to the kernel for processing
    3. **Pop** completions with `PopIoRingCompletion` - retrieve results as they complete
    
    This is similar to Linux's `io_uring` design and allows batching multiple operations for efficiency.

## Integration Options

There are three approaches to making VFS work with DirectStorage-enabled games, each with different trade-offs.

### Option 1: Force Legacy I/O Stack

!!! tip "Recommended for Determinism"
    
    The main benefit of this approach is determinism: it forces the same code path to run on Windows 10, Windows 11, and Wine.

Force DirectStorage to use the traditional Windows I/O stack that VFS already hooks. This is achieved by hooking DirectStorage's configuration APIs.

**How it works:**

1. Hook `DStorageSetConfiguration` to force the `ForceMappingLayer` flag (this ensures Win32 API usage)
2. Hook `DStorageGetFactory` to ensure `DStorageSetConfiguration` was called/configured before proceeding

```rust
use windows::Win32::Gaming::DirectStorage::*;

// Hook DStorageSetConfiguration to force legacy I/O
let mut config = DSTORAGE_CONFIGURATION::default();
config.ForceMappingLayer = true.into();
unsafe {
    DStorageSetConfiguration(&config)?;
}
```

**Trade-offs:**

| Aspect                    | Impact                                            |
| ------------------------- | ------------------------------------------------- |
| **Compatibility**         | ✅ Works on Windows 10, Windows 11, and Wine       |
| **Implementation Effort** | ✅ Minimal - just set flags                        |
| **Determinism**           | ✅ Same code path across all platforms             |
| **Performance**           | ⚠️ Loses IoRing performance benefits on Windows 11 |

!!! danger "My Opinion"
    
    Not recommended. Loses performance and is a DirectStorage-only solution.<br/>
    If you want to do it this way you're doing it ❌ WRONG.

### Option 2: Hook IoRing

!!! info "Use IoRing Where Supported"
    
    Use IoRing hooks where supported (Windows 11), and let the existing Win32 hooks handle when not supported (Windows 10, Wine).

!!! tip "Handles Any IoRing-Based Code"
    
    This way we handle any code based on IoRing, not just DirectStorage.

On Windows 11, DirectStorage uses IoRing for submission of I/O operations. By hooking IoRing's submission functions, VFS can intercept read requests and substitute virtual file data.

**How it works:**

Hook the following IoRing functions:

- `CreateIoRing` - Initialize tracking structures for this IoRing instance
- `BuildIoRingRegisterFileHandles` - Track registered virtual file handles for index lookups
- `BuildIoRingRegisterBuffers` - Track registered buffers for index lookups (optional)
- `BuildIoRingReadFile` - Detect virtual file operations and queue them
- `SubmitIoRing` - Process virtual operations and fill buffers
- `PopIoRingCompletion` - Return virtual completions before kernel completions
- `CloseIoRing` - Clean up tracking structures

!!! tip "Key Insight: Extract Handle and Buffer Directly from IoRing Refs"
    
    Both `IORING_HANDLE_REF` and `IORING_BUFFER_REF` contain direct values we need:
    
    ```c
    typedef struct IORING_HANDLE_REF {
        IORING_REF_KIND Kind;
        union {
            HANDLE Handle;  // ← The actual file handle
            UINT32 Index;   // ← For registered handles
        };
    } IORING_HANDLE_REF;
    
    typedef struct IORING_BUFFER_REF {
        IORING_REF_KIND Kind;
        union {
            void* Buffer;   // ← The actual buffer pointer
            UINT32 Index;   // ← For registered buffers
        };
    } IORING_BUFFER_REF;
    ```
    
    When `Kind == IORING_REF_RAW`, we can extract both the handle and buffer pointer directly. No need to track mappings!

!!! tip "High-Level Strategy"
    
    The core idea for handling virtual files with IoRing:
    
    1. **Intercept at build** - Detect virtual file operations in `BuildIoRingReadFile` and queue them instead of submitting to kernel
    2. **Synthesize at submit** - Process queued virtual operations in `SubmitIoRing` hook by copying data directly to buffers
    3. **Return completions** - Inject fake `IORING_CQE` completion entries via `PopIoRingCompletion` hook
    
    Since you cannot directly complete IoRing operations using public Windows APIs, we prevent kernel submission entirely for virtual files and handle everything in user space.

#### Implementation Guide

!!! note "The following is **pseudocode** demonstrating IoRing hooking for Layer 2 virtual file synthesis."

    No demo, time was lacking.

!!! note "API Naming"
    
    TODO: Replace these Win32 APIs with their NtDll counterparts.

##### Step 1: Define State Tracking

**Purpose:** Define state structures to track virtual file operations per IoRing instance.

**Design:** Each IoRing gets its own tracking structure containing all state for that ring. This ties the lifetime of our tracking data to the IoRing's lifetime.

```rust
use windows::Win32::System::IO::*;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

// Note: HashMap is still used for IORING_STATES (maps HIORING → IoRingVirtualState)
// but within IoRingVirtualState, we use Vec for sequential index lookups

// ========================================================================
// Per-IoRing State (tracks virtual file operations for one IoRing)
// ========================================================================

/// State for tracking virtual file operations in a single IoRing instance
struct IoRingVirtualState {
    /// Registered virtual file handles (indexed by registration order)
    /// Vec[index] = Some(HANDLE) if index is a virtual file, None otherwise
    /// Indices are sequential (0, 1, 2...), so Vec is more efficient than HashMap
    registered_virtual_handles: Vec<Option<HANDLE>>,
    
    /// Registered buffers (indexed by registration order)
    /// Vec[index] = buffer pointer
    /// Contains ALL registered buffers since we need to resolve any index
    /// when a virtual file operation uses IORING_REF_REGISTERED for the buffer
    /// Indices are sequential (0, 1, 2...), so Vec provides O(1) direct indexing
    registered_buffers: Vec<*mut u8>,
    
    /// Pending virtual file operations (queued during build, processed during submit)
    pending_operations: Vec<PendingVirtualOp>,
    
    /// Completed virtual operations (created during submit, returned during pop)
    completed_operations: VecDeque<IORING_CQE>,
}

impl IoRingVirtualState {
    fn new() -> Self {
        Self {
            registered_virtual_handles: Vec::new(),
            registered_buffers: Vec::new(),
            pending_operations: Vec::new(),
            completed_operations: VecDeque::new(),
        }
    }
}

/// Pending virtual file operation awaiting processing
#[derive(Clone)]
struct PendingVirtualOp {
    user_data: usize,       // User's tracking data (returned in completion)
    buffer_ptr: *mut u8,    // Where to write the synthesized data
    length: u32,            // How many bytes to read
    file_offset: u64,       // Offset within the virtual file
    file_handle: HANDLE,    // Which Layer 2 virtual file to read from
}

// ========================================================================
// Global Tracking (maps IoRing handle to its virtual state)
// ========================================================================

/// Maps each IoRing to its virtual file tracking state
/// Created in CreateIoRing hook, destroyed in CloseIoRing hook
static IORING_STATES: Mutex<HashMap<HIORING, IoRingVirtualState>> = 
    Mutex::new(HashMap::new());

// ========================================================================
// Layer 2 APIs Used
// ========================================================================
// See: docs/Virtual-FileSystem/Programmer-Usage/API-Reference.md
//
// - r3vfs_vfile_is_virtual_handle(HANDLE) -> bool
//     Checks if a handle is a Layer 2 virtual file
//
// - layer2_internal_read_virtual_file(HANDLE, offset, length) -> Vec<u8>
//     Internal Layer 2 function to synthesize virtual file data
//     (Same mechanism used by NtReadFile hooks)
```

##### Step 2: Hook Create/Close APIs (Lifetime Management)

**Purpose:** Tie the lifetime of our tracking structure to the IoRing's lifetime.

**When this runs:** When `CreateIoRing` creates an IoRing, and when `CloseIoRing` destroys it.

**Why this matters:** By hooking create and close we can tie the lifetime of the IoRing to our structure which holds extra data.

```rust
// ========================================================================
// Initialize Tracking State on IoRing Creation
// ========================================================================

fn hooked_create_ioring(
    ioring_version: IORING_VERSION,
    flags: IORING_CREATE_FLAGS,
    submission_queue_size: u32,
    completion_queue_size: u32,
    h: *mut HIORING,
) -> HRESULT {
    // Create the actual IoRing
    let result = original_create_ioring(
        ioring_version,
        flags,
        submission_queue_size,
        completion_queue_size,
        h,
    );
    
    if SUCCEEDED(result) {
        // IoRing was created successfully - initialize our tracking state
        let io_ring = unsafe { *h };
        let mut states = IORING_STATES.lock().unwrap();
        states.insert(io_ring, IoRingVirtualState::new());
    }
    
    result
}

// ========================================================================
// Clean Up Tracking State on IoRing Close
// ========================================================================

fn hooked_close_ioring(io_ring: HIORING) -> HRESULT {
    // Remove this IoRing's entire state
    // This cleans up everything: pending ops, completions, registered handles
    IORING_STATES.lock().unwrap().remove(&io_ring);
    
    // Close the actual ring
    original_close_ioring(io_ring)
}
```

##### Step 3: Hook Registration APIs

!!! info "Pre-Registration for Performance"
    
    Applications can pre-register file handles and buffers, then access them by index for performance reasons. This avoids kernel validation overhead on every operation.
    
    We need to track this: if any registered file is a virtual file, we must remember which index it corresponds to.

!!! note "DirectStorage Usage"
    
    DirectStorage doesn't use pre-registered handles or buffers - it uses direct references (`IORING_REF_RAW`). However, other applications might use registration, so we need to handle it.
    
    For handles, we only track virtual file indices. For buffers, we track ALL indices because we can't know ahead of time which buffers will be used with virtual files.

**Purpose:** Track only the indices of virtual file handles when registration happens.

**When this runs:** When applications call `BuildIoRingRegisterFileHandles` to pre-register handles for performance.

**Key insight:** We only track indices that correspond to Layer 2 virtual file handles. Real file handles are ignored.

**How it works:**

1. Application calls `BuildIoRingRegisterFileHandles([realHandle, virtualHandle, realHandle])`
2. Our hook pre-sizes a Vec to length 3: `[None, None, None]`
3. Check each handle by calling `r3vfs_vfile_is_virtual_handle()`
4. Only store virtual ones: `[None, Some(virtualHandle), None]`
5. Call original to actually register with kernel

Later, when an operation uses `IORING_REF_REGISTERED` with `Index=1`, we check `state.registered_virtual_handles[1]` and get `Some(virtualHandle)`.

Since indices are sequential (0, 1, 2...), Vec provides O(1) direct array indexing - much faster than HashMap hashing!

```rust
// ========================================================================
// Track ONLY Virtual File Handle Registrations
// ========================================================================

fn hooked_build_ioring_register_file_handles(
    io_ring: HIORING,
    handles: &[HANDLE],
    count: u32,
) -> HRESULT {
    // Get this IoRing's tracking state
    let mut states = IORING_STATES.lock().unwrap();
    if let Some(state) = states.get_mut(&io_ring) {
        // Pre-size the Vec to the exact count for O(1) indexed access
        state.registered_virtual_handles.clear();
        state.registered_virtual_handles.resize(count as usize, None);
        
        // Check each handle - only track virtual file handles
        for (index, &handle) in handles[..count as usize].iter().enumerate() {
            // Check if this is a Layer 2 virtual file handle
            if r3vfs_vfile_is_virtual_handle(handle) {
                // Yes - store the virtual file handle at this index
                state.registered_virtual_handles[index] = Some(handle);
            }
            // Real file handles remain None - they go through normal I/O
        }
    }
    drop(states);
    
    // Call original to actually register with kernel
    original_build_ioring_register_file_handles(io_ring, handles, count)
}

// ========================================================================
// Track Buffer Registrations
// ========================================================================
// We need to track ALL registered buffers, not just ones used with virtual files.
// Why? When a virtual file operation uses IORING_REF_REGISTERED for the buffer,
// we need to know what memory address that index points to so we can copy data.

fn hooked_build_ioring_register_buffers(
    io_ring: HIORING,
    buffers: &[IORING_BUFFER_INFO],
    count: u32,
) -> HRESULT {
    // Get this IoRing's tracking state
    let mut states = IORING_STATES.lock().unwrap();
    if let Some(state) = states.get_mut(&io_ring) {
        // Store all registered buffers in sequential order for O(1) indexed access
        state.registered_buffers.clear();
        state.registered_buffers.reserve(count as usize);
        
        for buffer_info in &buffers[..count as usize] {
            // Extract buffer pointer from IORING_BUFFER_INFO and append
            let buffer_ptr = buffer_info.Address as *mut u8;
            state.registered_buffers.push(buffer_ptr);
        }
    }
    drop(states);
    
    // Call original to actually register with kernel
    original_build_ioring_register_buffers(io_ring, buffers, count)
}
```

##### Step 4: Hook Build API (Detection Phase)

**Purpose:** Detect when read operations target Layer 2 virtual files and queue them for special handling.

**When this runs:** Every time `BuildIoRingReadFile` is called to queue a read operation.

**What happens:**

1. **Resolve file handle** - Extract from `IORING_HANDLE_REF`:
    - If `RAW`: Extract handle directly
    - If `REGISTERED`: Look up in our virtual handle tracking (from Step 3)
2. **Check if virtual** - Call `r3vfs_vfile_is_virtual_handle()` API
3. **If virtual:**
    - Resolve buffer pointer from `IORING_BUFFER_REF`:
        - If `RAW`: Extract pointer directly
        - If `REGISTERED`: Look up in our registered buffer tracking (from Step 3)
    - Queue the operation for processing during submit
    - Return success WITHOUT calling the original (prevents kernel submission)
4. **If real:**
    - Pass through to original API (normal kernel path - Layer 1 handles any redirects)

```rust
// ========================================================================
// Detect Virtual File Operations
// ========================================================================

fn hooked_build_ioring_read_file(
    io_ring: HIORING,
    file_ref: IORING_HANDLE_REF,        // Contains handle or index
    buffer_ref: IORING_BUFFER_REF,      // Contains pointer or index
    length: u32,
    file_offset: u64,
    user_data: usize,
    flags: IORING_SQE_FLAGS,
) -> HRESULT {
    // ---------------------------------------------------------------------
    // Resolve file handle (supports both raw and registered)
    // ---------------------------------------------------------------------
    let file_handle = match file_ref.Kind {
        IORING_REF_KIND::IORING_REF_RAW => {
            // Direct handle - extract it
            unsafe { file_ref.Anonymous.Handle }
        }
        IORING_REF_KIND::IORING_REF_REGISTERED => {
            // Registered handle - check if it's one of our virtual files
            let index = unsafe { file_ref.Anonymous.Index } as usize;
            let states = IORING_STATES.lock().unwrap();
            
            if let Some(state) = states.get(&io_ring) {
                // Direct Vec indexing - O(1) lookup
                if index < state.registered_virtual_handles.len() {
                    if let Some(handle) = state.registered_virtual_handles[index] {
                        // Yes, it's a virtual file we registered in Step 3
                        handle
                    } else {
                        // No, it's a real file (None) - pass through normally
                        return original_build_ioring_read_file(
                            io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                        );
                    }
                } else {
                    // Index out of bounds - shouldn't happen, pass through
                    return original_build_ioring_read_file(
                        io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                    );
                }
            } else {
                // IoRing state not found - shouldn't happen, pass through
                return original_build_ioring_read_file(
                    io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                );
            }
        }
        _ => {
            // Unknown reference kind - pass through
            return original_build_ioring_read_file(
                io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
            );
        }
    };

    // ---------------------------------------------------------------------
    // Check if this is a Layer 2 virtual file
    // ---------------------------------------------------------------------
    // For RAW mode, we still need to check. For REGISTERED, we already know.
    if r3vfs_vfile_is_virtual_handle(file_handle) {
        // Resolve buffer pointer
        let buffer_ptr = match buffer_ref.Kind {
            IORING_REF_KIND::IORING_REF_RAW => {
                // Direct pointer - extract it
                unsafe { buffer_ref.Anonymous.Buffer as *mut u8 }
            }
            IORING_REF_KIND::IORING_REF_REGISTERED => {
                // Registered buffer - direct Vec indexing for O(1) lookup
                let index = unsafe { buffer_ref.Anonymous.Index } as usize;
                let states = IORING_STATES.lock().unwrap();
                
                if let Some(state) = states.get(&io_ring) {
                    if index < state.registered_buffers.len() {
                        // Direct array indexing - found the registered buffer
                        state.registered_buffers[index]
                    } else {
                        // Buffer index out of bounds - shouldn't happen, pass through
                        return original_build_ioring_read_file(
                            io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                        );
                    }
                } else {
                    // IoRing state not found - shouldn't happen, pass through
                    return original_build_ioring_read_file(
                        io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                    );
                }
            }
            _ => {
                return original_build_ioring_read_file(
                    io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
                );
            }
        };

        // Queue this virtual file operation for processing at submit time
        let op = PendingVirtualOp {
            user_data,
            buffer_ptr,
            length,
            file_offset,
            file_handle,
        };
        
        let mut states = IORING_STATES.lock().unwrap();
        if let Some(state) = states.get_mut(&io_ring) {
            state.pending_operations.push(op);
        }
        drop(states);

        // Don't call original - prevents kernel submission
        return S_OK;
    }

    // Not a virtual file - pass through (Layer 1 handles real/redirected files)
    original_build_ioring_read_file(
        io_ring, file_ref, buffer_ref, length, file_offset, user_data, flags
    )
}
```

##### Step 5: Hook Submit API (Processing Phase)

**Purpose:** Process queued virtual file operations before submitting real operations to the kernel.

**When this runs:** When `SubmitIoRing` is called to submit batched operations.

**What happens:**

1. **Retrieve queued virtual operations** - Get all pending operations from this IoRing's state
2. **For each virtual operation:**
    - Call Layer 2's internal read mechanism to synthesize the data (synchronous)
    - Copy synthesized data directly to the application's buffer
    - Create a completion entry (`IORING_CQE`) with success status
    - Queue the completion in this IoRing's state for later retrieval
3. **Submit real operations** - Call original API to submit remaining (real file) operations to kernel

Virtual operations complete synchronously here, while real operations complete asynchronously in the kernel.

```rust
// ========================================================================
// Process Virtual Operations at Submit Time
// ========================================================================

fn hooked_submit_ioring(
    io_ring: HIORING,
    wait_operations: u32,
    milliseconds: u32,
    submitted: *mut u32,
) -> HRESULT {
    // Get this IoRing's state and extract pending operations
    let mut states = IORING_STATES.lock().unwrap();
    let pending_ops = if let Some(state) = states.get_mut(&io_ring) {
        // Take all pending operations (replaces with empty vec)
        std::mem::take(&mut state.pending_operations)
    } else {
        Vec::new()
    };
    drop(states);

    // Process each virtual file operation
    for op in pending_ops {
        // Synthesize data from the virtual file using Layer 2's internal read mechanism
        // Layer 2 looks up the FileHandler for this handle and calls its read() method
        // (Same mechanism used by NtReadFile hooks)
        let data = layer2_internal_read_virtual_file(
            op.file_handle, 
            op.file_offset, 
            op.length as usize
        );
        
        // Copy synthesized data to the application's buffer
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.as_ptr(),
                op.buffer_ptr,
                op.length as usize,
            );
        }

        // Create completion entry
        let cqe = IORING_CQE {
            UserData: op.user_data,
            ResultCode: S_OK.0,                   // Success
            Information: op.length as usize,      // Bytes transferred
        };

        // Queue completion in this IoRing's state
        let mut states = IORING_STATES.lock().unwrap();
        if let Some(state) = states.get_mut(&io_ring) {
            state.completed_operations.push_back(cqe);
        }
        drop(states);
    }

    // Submit remaining real file operations to kernel
    // Virtual operations were never added to the ring, so only real ops remain
    original_submit_ioring(io_ring, wait_operations, milliseconds, submitted)
}
```

##### Step 6: Hook Pop API (Completion Phase)

**Purpose:** Return completions to the application, prioritizing virtual completions first.

**When this runs:** When `PopIoRingCompletion` is called to retrieve completed operations.

**What happens:**

1. **Check for virtual completions** - Look in `COMPLETED_VIRTUAL_OPS` for this IoRing
2. **If virtual completions exist:**
    - Pop the next one from the queue (FIFO order)
    - Return it to the application
3. **If no virtual completions:**
    - Pass through to original API to get kernel completions

This ensures virtual file reads appear to complete immediately (since they were processed synchronously during submit).

```rust
// ========================================================================
// Return Virtual Completions
// ========================================================================

fn hooked_pop_ioring_completion(
    io_ring: HIORING,
    cqe_out: *mut IORING_CQE,
) -> HRESULT {
    // Check for virtual completions first
    let mut states = IORING_STATES.lock().unwrap();
    
    if let Some(state) = states.get_mut(&io_ring) {
        if let Some(virtual_cqe) = state.completed_operations.pop_front() {
            unsafe {
                *cqe_out = virtual_cqe;
            }
            return S_OK;  // Completion available
        }
    }
    drop(states);

    // No virtual completions - get real completions from kernel
    original_pop_ioring_completion(io_ring, cqe_out)
}
```

**Trade-offs:**

| Aspect                    | Impact                                                                  |
| ------------------------- | ----------------------------------------------------------------------- |
| **Compatibility**         | ✅ Solves DirectStorage everywhere (but other IoRing uses only on Win11) |
| **Implementation Effort** | ⚠️ Moderate - requires hooking and buffer management                     |
| **Performance**           | ✅ Preserves DirectStorage benefits for real files                       |

!!! info "Implementation Notes"
    
    **Layer 2 Integration:** This implementation uses Layer 2's `r3vfs_vfile_is_virtual_handle()` API to check if handles are virtual. For reading, it calls Layer 2's internal read mechanism (same mechanism used by `NtReadFile` hooks).
    
    **Completion Order:** Virtual file completions are returned in FIFO order before kernel completions. Virtual files complete synchronously at `SubmitIoRing` time, while real files complete asynchronously in the kernel.
    
    **Registered Resources:** The implementation supports both `IORING_REF_RAW` (direct handles/pointers) and `IORING_REF_REGISTERED` (indexed handles/buffers). Since indices are sequential (0, 1, 2...), we use `Vec` instead of `HashMap` for O(1) direct array indexing. For handles, we store `Vec<Option<HANDLE>>` (only virtual files are Some). For buffers, we store `Vec<*mut u8>` (all buffers, needed for index resolution).
    
    **DirectStorage Usage:** DirectStorage uses raw references (`IORING_REF_RAW`), not registered resources. However, other applications may use registration, so the implementation handles both modes.
    
    **Thread Safety:** IoRing can be accessed from multiple threads. The `Mutex` guards ensure thread safety, though in practice each IoRing is typically single-threaded.

!!! warning "Prototype Implementation / Proof of Concept"
    
    The above is a conceptual prototype.
    
    In the final version we'd strive to make Layer 2's read mechanism truly asynchronous rather than blocking during `SubmitIoRing`; so the whole pipeline stays asynchronous end-to-end.

## Recommended Strategy

!!! tip "Implementation Approach"
    
    **For Initial Testing:**
    
    You can use **Option 1 (Force Legacy I/O)** for quick initial testing. It's the quickest to implement and easiest to debug.
    
    **For Release:**
    
    You **must** use **Option 2 (Hook IoRing)** for production releases. This preserves DirectStorage's performance benefits and works everywhere (IoRing on Win11, falls back to existing Win32 hooks on Win10/Wine).

[directstorage-dataflow]: https://github.com/microsoft/DirectStorage/blob/main/Docs/DeveloperGuidance.md#uncompressed-data-flow
[ioring-docs]: https://windows-internals.com/ioring-vs-io_uring-a-comparison-of-windows-and-linux-implementations/
