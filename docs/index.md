!!! note "Attribution"
	**[Forked from the Reloaded-III Specification](https://reloaded-project.github.io/Reloaded-III/Mods/Essentials/Virtual-FileSystem/About.html)**
	
	The R3 spec will either link here in the future, or this will be upstreamed to the spec.

# User Space Virtual Filesystems

User Space Virtual Filesystems are an invisible layer that sits between applications and the operating system's file operations. They allow applications to seamlessly work with files that don't actually exist on disk, enabling powerful functionality like file redirection, merging, and emulation without modifying the original folder structure or requiring administrator privileges.

At the core of User Space Virtual Filesystems is the concept of **API hooking**—intercepting the Windows API calls that applications make when opening, reading, or accessing files. By hooking these low-level operations, we can transparently redirect file access to different locations, synthesize files on-the-fly, or merge multiple sources without the application ever knowing the difference. This works at a fundamental level, making it compatible with virtually any application that reads files.

This documentation covers two complementary technologies for implementing User Space Virtual Filesystems: the **File Emulation Framework**, which intercepts OS API calls to create virtual files on demand, and the **Reloaded Virtual FileSystem**, which provides transparent file redirection and merging capabilities. Together, they enable robust solutions for modding, testing, and file virtualization.

## Documentation Sections

### File Emulation Framework

The File Emulation Framework is a framework for intercepting Operating System API calls related to file reading, allowing you to trick applications into loading files that don't really exist. Perfect for dynamic file creation and archive manipulation without modifying disk contents.

**[→ Explore File Emulation Framework](File-Emulation-Framework/About.md)**

### Virtual FileSystem

The Reloaded Virtual FileSystem is an invisible helper that sits between your applications and the files they use. It allows applications to 'see' and open files that aren't really 'there', keeping your folders unmodified while enabling seamless file merging and redirection.

**[→ Explore Virtual FileSystem](Virtual-FileSystem/About.md)**
