# Hooking Implementation Overview

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


