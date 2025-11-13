# Agent Guidelines: VFS Documentation & Rust Examples

## About This Project

This repository documents a **two-layer Virtual FileSystem (VFS) architecture** for game modding. The VFS allows games to see and access files that don't physically exist in the game folder, enabling non-invasive modding without administrator rights or filesystem modifications.

**Layer 1** (Virtual FileSystem) handles path redirection and makes virtual files visible in directory listings by hooking OS file APIs (primarily `ntdll.dll` on Windows).

**Layer 2** (Virtual File Framework) synthesizes file content on-the-fly when virtual files are read, providing an abstraction for extensions.

**Layer 3** (Extensions) are plugins like Archive Emulation Framework and Nx2VFS that build on Layers 1 & 2.

The `docs/` folder contains MkDocs documentation. The `examples/` folder contains Rust demonstration code showcasing VFS-related techniques (DirectStorage, memory mapping, PE patching).

## Language

Use **British English** spelling throughout all documentation, code comments, and error messages.

## Commands

- `mkdocs build --strict` — Build documentation and check for errors
- Before running mkdocs, create/use a virtual environment like `start_docs.py` does
- **NEVER run blocking commands** like `mkdocs serve` or `start_docs.py`
- The `examples/` folder contains a Cargo workspace with Rust demonstration code

## Core Documentation Invariants

- **Visuals with captions**: Use `.avif` where visuals add clarity; include a caption
- **Link hygiene**: Prefer relative links for internal docs; link to official tool docs externally
- **Admonitions for emphasis**: Use `tip`, `info`, `warning`, `example`, and collapsible `???` blocks

## Visuals

- Prefer `.avif` images with descriptive alt text and captions
- Include visuals when they clarify outcomes; skip them for configuration pages if not valuable

Caption format:
```markdown
![Descriptive alt text](../assets/image.avif)
/// caption
Clear explanation of what's shown and why it matters
///
```

## Admonitions

Use for emphasis and scannability:
```markdown
!!! tip "Best Practice"
    Helpful hint for optimal usage.

!!! info "Additional Context"
    Extra useful information.

!!! warning "Important Note"
    Critical information.

!!! example "Real World Use"
    Practical example.

??? info "Advanced Details"
    Collapsible technical details.
```

## Annotations

Inline and list annotations are allowed:
```markdown
Text with annotation (1)
{ .annotate }

1.  This is the annotation content.
```

For lists:
```markdown
<div class="annotate" markdown>

1. item one (1)
2. item two

</div>

1. annotation for item one
```

## Technical Code Blocks

Always specify language for syntax highlighting:
```markdown
​```bash
cargo bench
​```

​```rust
fn example() {}
​```
```

## File Organisation

- `docs/` markdown files follow `mkdocs.yml` navigation structure
- `examples/` is a Cargo workspace; each example is a separate package
