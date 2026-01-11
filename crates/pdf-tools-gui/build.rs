use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Only run for non-WASM targets with pdf-viewer feature
    let target = env::var("TARGET").unwrap();
    if target.contains("wasm32") {
        return;
    }

    // Check if pdf-viewer feature is enabled
    let has_pdf_viewer = env::var("CARGO_FEATURE_PDF_VIEWER").is_ok();
    if !has_pdf_viewer {
        return;
    }

    // Use pdfium_7543 (latest stable as of pdfium-render 0.8.37)
    let pdfium_version = "chromium/7543";

    // Determine platform and architecture
    let (platform, arch, lib_name) = match target.as_str() {
        t if t.contains("apple") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else {
                "x64"
            };
            ("mac", arch, "libpdfium.dylib")
        }
        t if t.contains("linux") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else {
                "x64"
            };
            ("linux", arch, "libpdfium.so")
        }
        t if t.contains("windows") => {
            let arch = if t.contains("aarch64") {
                "arm64"
            } else if t.contains("i686") {
                "x86"
            } else {
                "x64"
            };
            ("win", arch, "pdfium.dll")
        }
        _ => {
            println!("cargo:warning=Unsupported target platform: {}", target);
            return;
        }
    };

    // Set up paths relative to the repository root
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root");
    let pdfium_dir = workspace_root.join("vendor").join("pdfium");
    let lib_dir = pdfium_dir.join("lib");
    let include_dir = pdfium_dir.join("include");
    let lib_path = lib_dir.join(lib_name);

    // Check if already downloaded
    if lib_path.exists() {
        println!(
            "cargo:warning=PDFium already exists at {}",
            lib_path.display()
        );
        fix_library_install_name(&lib_path, platform);
        configure_linking(&lib_dir, &include_dir);
        return;
    }

    println!(
        "cargo:warning=Downloading PDFium {} for {}-{}",
        pdfium_version, platform, arch
    );

    // Create directories
    fs::create_dir_all(&lib_dir).expect("Failed to create lib directory");
    fs::create_dir_all(&include_dir).expect("Failed to create include directory");

    // Download URL
    let download_url = format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/download/{}/pdfium-{}-{}.tgz",
        pdfium_version, platform, arch
    );

    // Download and extract
    let temp_file = env::temp_dir().join("pdfium.tgz");

    println!("cargo:warning=Downloading from {}", download_url);
    download_file(&download_url, &temp_file);

    println!("cargo:warning=Extracting to {}", pdfium_dir.display());
    extract_tarball(&temp_file, &pdfium_dir);

    // Clean up
    let _ = fs::remove_file(&temp_file);

    // Verify installation
    if !lib_path.exists() {
        panic!(
            "PDFium installation failed: {} not found",
            lib_path.display()
        );
    }

    println!(
        "cargo:warning=PDFium installed successfully to {}",
        pdfium_dir.display()
    );

    // Fix install name on macOS
    fix_library_install_name(&lib_path, platform);

    configure_linking(&lib_dir, &include_dir);
}

fn configure_linking(lib_dir: &Path, include_dir: &Path) {
    let target = env::var("TARGET").unwrap();

    // Tell cargo to link against pdfium
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=pdfium");

    // Set rpath so the binary can find the library at runtime
    if target.contains("apple") {
        // macOS: set rpath relative to executable or to the vendor directory
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    } else if target.contains("linux") {
        // Linux: set rpath relative to executable or to the vendor directory
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    }

    // Tell cargo to expose include directory
    println!("cargo:include={}", include_dir.display());

    // Rerun if the library changes
    println!("cargo:rerun-if-changed={}", lib_dir.display());
}

fn download_file(url: &str, dest: &Path) {
    use std::io::Write;

    let response = ureq::get(url)
        .call()
        .unwrap_or_else(|e| panic!("Failed to download {}: {}", url, e));

    let mut file = fs::File::create(dest).expect("Failed to create temp file");
    std::io::copy(&mut response.into_reader(), &mut file).expect("Failed to write download");
    file.flush().expect("Failed to flush file");
}

fn extract_tarball(tarball: &Path, dest: &Path) {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let tar_gz = fs::File::open(tarball).expect("Failed to open tarball");
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(dest).expect("Failed to extract tarball");
}

fn fix_library_install_name(lib_path: &Path, platform: &str) {
    if platform != "mac" {
        return;
    }

    // On macOS, fix the install name to use @rpath
    let output = std::process::Command::new("install_name_tool")
        .arg("-id")
        .arg("@rpath/libpdfium.dylib")
        .arg(lib_path)
        .output();

    match output {
        Ok(result) if result.status.success() => {
            println!(
                "cargo:warning=Fixed install name for {}",
                lib_path.display()
            );
        }
        Ok(result) => {
            println!(
                "cargo:warning=Failed to fix install name: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        Err(e) => {
            println!("cargo:warning=install_name_tool not available: {}", e);
        }
    }
}
