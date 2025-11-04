# Agent Guidelines: Documentation Folder

## Documentation Commands
- `mkdocs build --strict` — Check build errors
- NEVER run blocking commands like `mkdocs serve`
- Before running mkdocs, make/use a venv like `start_docs.py` does.

## Core Invariants
- **Visuals with captions**: Use `.avif` where visuals add clarity; include a caption.
- **Link hygiene**: Prefer relative links for internal docs; link to official tool docs externally.
- **Admonitions for emphasis**: Use `tip`, `info`, `warning`, `example`, and collapsible `???` blocks.

## Visuals
- Prefer `.avif` images with descriptive alt text and captions.
- Include visuals when they clarify outcomes; skip them for configuration pages if not valuable.

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

- **Technical code blocks**
```markdown
```bash
cargo bench
```

```rust
fn example() {}
```
```