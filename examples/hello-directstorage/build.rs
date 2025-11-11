use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Rerun this build script if the libs directory changes
    println!("cargo:rerun-if-changed=libs");

    // Get various build-related paths
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");

    let out_path = PathBuf::from(&out_dir);
    let manifest_path = PathBuf::from(&manifest_dir);

    // Calculate the binary output directory
    // OUT_DIR is target/{profile}/build/{package}-{hash}/out
    // We need to go up 3 levels to reach target/{profile}
    let binary_dir = out_path
        .parent() // target/{profile}/build/{package}-{hash}
        .and_then(|p| p.parent()) // target/{profile}/build
        .and_then(|p| p.parent()) // target/{profile}
        .expect("Could not determine target directory");

    // Source directory for DLLs (relative to Cargo.toml)
    let libs_dir = manifest_path.join("libs");

    if libs_dir.exists() {
        if let Ok(entries) = fs::read_dir(&libs_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "dll") {
                    if let Some(file_name) = path.file_name() {
                        let dest = binary_dir.join(file_name);
                        match fs::copy(&path, &dest) {
                            Ok(_) => {
                                println!(
                                    "cargo:warning=DLL {} -> {}",
                                    file_name.to_string_lossy(),
                                    dest.display()
                                );
                            }
                            Err(e) => {
                                println!(
                                    "cargo:warning=Failed to copy {}: {}",
                                    file_name.to_string_lossy(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    // Also output the directory path so it can be used if needed
    println!("cargo:rustc-env=DLL_SEARCH_PATH={}", binary_dir.display());
}
