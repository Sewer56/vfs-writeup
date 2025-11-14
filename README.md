# VFS Writeup

Documentation for implementing a two-layer Virtual FileSystem architecture for non-invasive game modding.

## Documentation

Full documentation is available at: **https://sewer56.dev/vfs-writeup/**

## What is this?

This project documents a production-tested VFS implementation that allows games to see and access files that don't physically exist in the game folder, enabling modding without administrator rights or filesystem modifications.

- **Layer 1**: Virtual FileSystem - Path redirection and virtual file visibility
- **Layer 2**: Virtual File Framework - On-the-fly content synthesis
- **Layer 3**: Extensions - Archive emulation and other plugins

## Local Development

```bash
python start_docs.py
```

Then visit http://localhost:8000
