# Memory Map Pre-Population Demo

Demonstrates the pre-population strategy for memory-mapped virtual files by allocating and populating memory upfront during section creation, eliminating page faults entirely.

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
cargo run --bin mmap-pre-populate
```

## Reference

See [Memory-Map-Hooking.md](../../docs/Technology-Integrations/Memory-Map-Hooking.md#strategy-1-pre-population-small-files) for detailed technical explanation of the pre-population strategy.
