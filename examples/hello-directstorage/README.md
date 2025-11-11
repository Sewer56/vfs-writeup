# HelloDirectStorage Demo

A Rust port of Microsoft's HelloDirectStorage sample, demonstrating the basic DirectStorage API workflow for high-performance file I/O on Windows.

## Overview

This demo showcases how to use the DirectStorage API to load files directly into GPU memory with minimal CPU overhead. DirectStorage is a Windows feature designed for high-performance game asset loading and can bypass traditional I/O bottlenecks.

## Prerequisites

- **Windows 10 version 1909 or later** (DirectStorage requires this minimum version)
- **DirectX 12 support** (D3D12-capable GPU)
- **Rust toolchain** (stable channel recommended)

## Setup

**No manual setup required!** The DirectStorage DLLs (v1.3.0) are already included in the project under `libs/`, and the build script automatically copies them to the correct location when you build the demo.

The following DLL files are pre-included:
- `dstorage.dll` (DirectStorage API)
- `dstoragecore.dll` (DirectStorage core runtime)

Simply build and run—everything is configured to work out of the box on Windows 10 1909+.

## Usage

```bash
# Load Cargo.toml via DirectStorage
cargo run --bin hello-directstorage -- Cargo.toml
```

The demo will:
1. Initialise the DirectStorage factory
2. Open the specified file via DirectStorage API
3. Create a D3D12 device and GPU buffer
4. Create a DirectStorage queue for asynchronous operations
5. Enqueue a read request to load the file into the GPU buffer
6. Wait for completion using fence synchronisation
7. Report the operation status and any errors

## Reference

Based on the original C++ sample:
- [Microsoft DirectStorage HelloDirectStorage Sample](https://github.com/microsoft/DirectStorage/tree/main/Samples/HelloDirectStorage)
- [DirectStorage Documentation](https://docs.microsoft.com/en-us/gaming/gdk/_content/gc/system/overviews/directstorage/directstorage-overview)

## Notes

- DirectStorage APIs are Windows-exclusive and require appropriate hardware support
- The demo uses a GPU-local default heap (`D3D12_HEAP_TYPE_DEFAULT`) for optimal DirectStorage performance