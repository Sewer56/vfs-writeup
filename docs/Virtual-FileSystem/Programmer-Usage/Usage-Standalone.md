# Usage as Standalone Library

!!! warning "This is a plan/prototype - finer details may change during development"

    All APIs shown here are for reference only and may be refined during implementation.

For standalone usage (without an existing integration, like a Reloaded3 mod), the VFS provides a C API that can be used from any language.

## Initialization & Lifecycle

!!! info "Only needed for standalone usage"

    When using via an existing integration (such as a Reloaded3 mod), initialization is handled automatically by the integration.

```c
// Initialize the VFS system
R3VfsResult r3vfs_init(void);

// Shutdown and cleanup all VFS resources
void r3vfs_shutdown(void);
```

---

!!! tip "For complete API documentation"

    Once you've initialized the library, you may use it as instructed in the [API Reference](API-Reference.md).

