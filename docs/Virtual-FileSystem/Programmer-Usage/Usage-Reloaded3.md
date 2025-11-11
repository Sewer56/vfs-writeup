# Usage from Reloaded3

!!! warning "This is a plan/prototype - finer details may change during development"

    All APIs shown here are for reference only and may be refined during implementation.

The VFS uses the Reloaded Dependency Injection [TODO: Link Pending] system to expose an API.

To use the VFS API from Reloaded3:

1. Add the `reloaded3.api.windows.vfs.interfaces.s56` (or equivalent for your language) package to your project.

2. Add the dependency `reloaded3.api.windows.vfs.s56` to your mod's dependencies.

3. In your mod's entry point, acquire the necessary services:

!!! note "Services are registered by name"

    Under the hood, these services are registered by name in the dependency injection container. The type-safe API is a convenience wrapper around name-based service lookup.

=== "Rust"
    ```rust
    struct MyMod {
        redirector: Option<Redirector>,
        virtual_files: Option<VirtualFiles>,
        settings: Option<Settings>,
    }

    impl MyMod {
        fn new(loader: &mut IModLoader) -> Self {
            Self {
                redirector: loader.get_service::<Redirector>().ok(),
                virtual_files: loader.get_service::<VirtualFiles>().ok(),
                settings: loader.get_service::<Settings>().ok(),
            }
        }
    }
    ```

=== "C++"
    ```cpp
    class MyMod {
    private:
        Redirector* _redirector;
        VirtualFiles* _virtualFiles;
        Settings* _settings;

    public:
        MyMod(IModLoader* loader)
        {
            _redirector = loader->GetService<Redirector>();
            _virtualFiles = loader->GetService<VirtualFiles>();
            _settings = loader->GetService<Settings>();
        }
    };
    ```

=== "C#"
    ```csharp
    IRedirector _redirector;
    IVirtualFiles _virtualFiles;
    ISettings _settings;

    public void Start(IModLoaderV1 loader)
    {
        _redirector = _modLoader.GetService<IRedirector>();
        _virtualFiles = _modLoader.GetService<IVirtualFiles>();
        _settings = _modLoader.GetService<ISettings>();
    }
    ```

!!! info "Initialization handled automatically"

    When using the Reloaded3 mod integration, the VFS is automatically initialized by the VFS mod itself (which you've added as a dependency).
    You do not need to call any initialization or shutdown functions.

---

!!! tip "For complete API documentation"

    Once you've acquired the services, you may use them as instructed in the [API Reference](API-Reference.md).

