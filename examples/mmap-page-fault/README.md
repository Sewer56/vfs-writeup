# Memory Map Page Fault Emulation Demo

Demonstrates the page fault emulation strategy for memory-mapped virtual files by reserving address space upfront and committing individual pages on-demand as they are accessed.

## Prerequisites

Requires Rust toolchain. Install from [https://rust-lang.org/tools/install/](https://rust-lang.org/tools/install/).

Not tested on Wine ATM (in case `NtUnmapViewOfSectionEx` etc. variant doesn't exist, etc.), this is just a quick one off made on Win11 25H2 to prove the concept.

## File Structure

- `src/main.rs` — Demo entry point and workflow
- `src/hooks.rs` — Five-hook implementation (NtCreateSection, NtMapViewOfSection, NtUnmapViewOfSection, NtUnmapViewOfSectionEx, NtClose)
- `src/content.rs` — Virtual file content synthesis
- `src/nt_types.rs` — NT API type definitions

## Usage

```bash
cargo run --bin mmap-page-fault
```

## Reference

See [Memory-Map-Hooking.md](../../docs/Technology-Integrations/Memory-Map-Hooking.md#strategy-2-page-fault-emulation-large-files) for detailed technical explanation of the page fault emulation strategy.
