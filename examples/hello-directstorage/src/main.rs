//! HelloDirectStorage Demo
//!
//! This is a Rust port of Microsoft's HelloDirectStorage sample, demonstrating
//! the basic DirectStorage API workflow for high-performance file I/O on Windows.
//!
//! The demo performs the following steps:
//! 1. Initialises the DirectStorage factory
//! 2. Opens a file using the DirectStorage API
//! 3. Creates a D3D12 device and GPU buffer
//! 4. Creates a DirectStorage queue for asynchronous operations
//! 5. Enqueues a read request to load the file into the GPU buffer
//! 6. Waits for the operation to complete using fence synchronisation
//! 7. Reports any errors encountered during the process
//!
//! # Requirements
//! - Windows 10 version 1909 or later
//! - DirectStorage SDK 1.0+
//! - DirectX 12 support
//!
//! # Usage
//! ```
//! hello-directstorage <filepath>
//! ```

#![warn(missing_docs)]

use std::mem::ManuallyDrop;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use direct_storage::{
    readonly_copy, runtime_loaded::DStorageGetFactory, IDStorageFactory, IDStorageFile,
    IDStorageQueue, DSTORAGE_COMPRESSION_FORMAT_NONE, DSTORAGE_DESTINATION,
    DSTORAGE_DESTINATION_BUFFER, DSTORAGE_MAX_QUEUE_CAPACITY, DSTORAGE_PRIORITY_NORMAL,
    DSTORAGE_QUEUE_DESC, DSTORAGE_REQUEST, DSTORAGE_REQUEST_DESTINATION_BUFFER,
    DSTORAGE_REQUEST_OPTIONS, DSTORAGE_REQUEST_SOURCE_FILE, DSTORAGE_SOURCE, DSTORAGE_SOURCE_FILE,
};
use windows::{
    core::{PCSTR, PCWSTR},
    Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        Graphics::{
            Direct3D::D3D_FEATURE_LEVEL_11_0,
            Direct3D12::{
                D3D12CreateDevice, ID3D12Device, ID3D12Resource, D3D12_FENCE_FLAG_NONE,
                D3D12_HEAP_FLAG_NONE, D3D12_HEAP_PROPERTIES, D3D12_HEAP_TYPE_DEFAULT,
                D3D12_RESOURCE_DESC, D3D12_RESOURCE_DIMENSION_BUFFER, D3D12_RESOURCE_STATE_COMMON,
                D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            },
            Dxgi::Common::{DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC},
        },
        Storage::FileSystem::BY_HANDLE_FILE_INFORMATION,
        System::Threading::{CreateEventW, WaitForSingleObject},
    },
};

fn main() {
    // Parse command-line arguments
    let file_path = match parse_args() {
        Ok(path) => path,
        Err(msg) => {
            eprintln!("{}", msg);
            std::process::exit(1);
        }
    };

    println!("HelloDirectStorage - Loading file: {}", file_path.display());

    // Check if file exists
    if !file_path.exists() {
        eprintln!("Error: File not found: {}", file_path.display());
        std::process::exit(1);
    }

    // Initialise DirectStorage and run the demo
    if let Err(e) = run_demo(&file_path) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

/// Parses command-line arguments to extract the file path
fn parse_args() -> Result<PathBuf, String> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        return Err(format!(
            "Usage: {} <filepath>\nProvide a file path to load via DirectStorage",
            args.first()
                .map(|s| s.as_str())
                .unwrap_or("hello-directstorage")
        ));
    }

    Ok(PathBuf::from(&args[1]))
}

/// Main demo function that runs the DirectStorage workflow
fn run_demo(file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== DirectStorage Demo ===\n");

    // Step 1: Initialise DirectStorage factory
    println!("[1/7] Initialising DirectStorage factory...");
    let factory = initialise_directstorage()?;
    println!("      ✓ DirectStorage factory created\n");

    // Step 2: Open file via DirectStorage API
    println!("[2/7] Opening file via DirectStorage API...");
    let file = open_file(&factory, file_path)?;
    println!("      ✓ File opened successfully\n");

    // Get file size
    let file_size = get_file_size(&file)?;
    println!("      File size: {} bytes\n", file_size);

    // Step 3: Create D3D12 device
    println!("[3/7] Creating D3D12 device...");
    let device = create_d3d12_device()?;
    println!("      ✓ D3D12 device created\n");

    // Step 4: Create GPU buffer
    println!("[4/7] Creating GPU buffer...");
    let buffer = create_gpu_buffer(&device, file_size)?;
    println!("      ✓ GPU buffer created ({} bytes)\n", file_size);

    // Step 5: Create DirectStorage queue
    println!("[5/7] Creating DirectStorage queue...");
    let queue = create_directstorage_queue(&factory, &device)?;
    println!("      ✓ DirectStorage queue created\n");

    // Step 6: Enqueue read request
    println!("[6/7] Enqueueing read request...");
    enqueue_read_request(&queue, &file, &buffer, file_size)?;
    println!("      ✓ Read request enqueued\n");

    // Step 7: Wait for completion and check errors
    println!("[7/7] Waiting for DirectStorage request to complete...");
    wait_for_completion(&queue, &device)?;
    println!("      ✓ Request completed\n");

    // Check for errors
    check_error_records(&queue)?;

    println!("=== Operation Complete ===\n");
    Ok(())
}

/// Initialises the DirectStorage factory
fn initialise_directstorage() -> Result<IDStorageFactory, Box<dyn std::error::Error>> {
    unsafe {
        let factory =
            DStorageGetFactory().map_err(|_| "Failed to initialise DirectStorage factory")?;
        Ok(factory)
    }
}

/// Opens a file using the DirectStorage API
fn open_file(
    factory: &IDStorageFactory,
    path: &Path,
) -> Result<IDStorageFile, Box<dyn std::error::Error>> {
    // Convert path to UTF-16 wide string (required by Windows API)
    let path_wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let file = factory
            .OpenFile(PCWSTR::from_raw(path_wide.as_ptr()))
            .map_err(|_| "Failed to open file via DirectStorage API")?;
        Ok(file)
    }
}

/// Gets the file size from DirectStorage file information
fn get_file_size(file: &IDStorageFile) -> Result<u64, Box<dyn std::error::Error>> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        file.GetFileInformation(&mut info)
            .map_err(|_| "Failed to get file information")?;
    }
    let size = ((info.nFileSizeHigh as u64) << 32) | (info.nFileSizeLow as u64);
    Ok(size)
}

/// Creates a D3D12 device for GPU operations
fn create_d3d12_device() -> Result<ID3D12Device, Box<dyn std::error::Error>> {
    unsafe {
        let mut device = None;
        D3D12CreateDevice(None, D3D_FEATURE_LEVEL_11_0, &mut device)
            .map_err(|_| "Failed to create D3D12 device")?;
        device.ok_or("D3D12 device creation returned None".into())
    }
}

/// Creates a GPU buffer resource for reading file data into
fn create_gpu_buffer(
    device: &ID3D12Device,
    size: u64,
) -> Result<ID3D12Resource, Box<dyn std::error::Error>> {
    unsafe {
        // Configure heap properties for default (GPU-local) heap
        let heap_props = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            ..Default::default()
        };

        // Configure buffer resource description
        let desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_BUFFER,
            Alignment: 0,
            Width: size,
            Height: 1,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_UNKNOWN,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_ROW_MAJOR,
            Flags: Default::default(),
        };

        // Create the committed resource
        let mut resource = None;
        device
            .CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_NONE,
                &desc,
                D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )
            .map_err(|_| "Failed to create committed resource")?;
        resource.ok_or("GPU buffer creation returned None".into())
    }
}

/// Creates a DirectStorage queue for asynchronous I/O operations
fn create_directstorage_queue(
    factory: &IDStorageFactory,
    device: &ID3D12Device,
) -> Result<IDStorageQueue, Box<dyn std::error::Error>> {
    let queue_desc = DSTORAGE_QUEUE_DESC {
        SourceType: DSTORAGE_REQUEST_SOURCE_FILE,
        Capacity: DSTORAGE_MAX_QUEUE_CAPACITY as u16,
        Priority: DSTORAGE_PRIORITY_NORMAL,
        Name: PCSTR::null(),
        Device: unsafe { readonly_copy(device) },
    };

    unsafe {
        let queue = factory
            .CreateQueue(&queue_desc)
            .map_err(|_| "Failed to create DirectStorage queue")?;
        Ok(queue)
    }
}

/// Enqueues a read request to load file data into the GPU buffer
fn enqueue_read_request(
    queue: &IDStorageQueue,
    file: &IDStorageFile,
    buffer: &ID3D12Resource,
    size: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if file size exceeds DirectStorage API limits (u32::MAX)
    if size > u32::MAX as u64 {
        return Err(format!(
            "File size ({} bytes) exceeds DirectStorage API limit of {} bytes",
            size,
            u32::MAX
        )
        .into());
    }
    let size_u32 = size as u32;
    let mut options = DSTORAGE_REQUEST_OPTIONS::default();
    options.set_CompressionFormat(DSTORAGE_COMPRESSION_FORMAT_NONE);
    options.set_SourceType(DSTORAGE_REQUEST_SOURCE_FILE);
    options.set_DestinationType(DSTORAGE_REQUEST_DESTINATION_BUFFER);

    let request = DSTORAGE_REQUEST {
        Options: options,
        Source: DSTORAGE_SOURCE {
            File: ManuallyDrop::new(DSTORAGE_SOURCE_FILE {
                Source: unsafe { readonly_copy(file) },
                Offset: 0,
                Size: size_u32,
            }),
        },
        Destination: DSTORAGE_DESTINATION {
            Buffer: ManuallyDrop::new(DSTORAGE_DESTINATION_BUFFER {
                Resource: unsafe { readonly_copy(buffer) },
                Offset: 0,
                Size: size_u32,
            }),
        },
        UncompressedSize: size_u32,
        CancellationTag: 0,
        Name: PCSTR::null(),
    };

    unsafe {
        queue.EnqueueRequest(&request);
    }

    Ok(())
}

/// Waits for all queued operations to complete using fence synchronisation
fn wait_for_completion(
    queue: &IDStorageQueue,
    device: &ID3D12Device,
) -> Result<(), Box<dyn std::error::Error>> {
    const FENCE_VALUE: u64 = 1;

    unsafe {
        // Create fence for synchronisation
        let fence: windows::Win32::Graphics::Direct3D12::ID3D12Fence = device
            .CreateFence(0, D3D12_FENCE_FLAG_NONE)
            .map_err(|_| "Failed to create fence")?;

        // Create event for signalling
        let event = CreateEventW(None, false, false, None).map_err(|_| "Failed to create event")?;

        // Configure fence to signal event on completion
        fence
            .SetEventOnCompletion(FENCE_VALUE, event)
            .map_err(|_| "Failed to set event on completion")?;

        // Enqueue signal operation and submit
        queue.EnqueueSignal(&fence, FENCE_VALUE);
        queue.Submit();

        // Wait for completion (5 second timeout)
        let wait_result = WaitForSingleObject(event, 5 * 1000);

        // Clean up event handle
        if event != INVALID_HANDLE_VALUE {
            CloseHandle(event).ok();
        }

        // Check wait result
        match wait_result {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_TIMEOUT => Err("DirectStorage operation timed out after 5 seconds".into()),
            WAIT_FAILED => Err("WaitForSingleObject failed".into()),
            _ => Err(format!("Unexpected wait result: {:?}", wait_result).into()),
        }
    }
}

/// Checks and reports any DirectStorage error records
fn check_error_records(queue: &IDStorageQueue) -> Result<(), Box<dyn std::error::Error>> {
    let error_record = unsafe { queue.RetrieveErrorRecord() };

    if error_record.FailureCount > 0 {
        return Err(format!(
            "DirectStorage request failed. Error count: {}, HRESULT: {:?}",
            error_record.FailureCount, error_record.FirstFailure.HResult
        )
        .into());
    }

    println!("✓ No errors detected in DirectStorage operations\n");
    Ok(())
}
