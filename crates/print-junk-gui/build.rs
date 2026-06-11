use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root");

    // Generate icon assets from source image
    generate_icons(workspace_root, &manifest_dir);

    // Embed icon and version info in Windows executables
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "Print Junk");
        res.set(
            "FileDescription",
            "Print Junk - Imposition and processing for printers",
        );
        res.compile().expect("Failed to compile Windows resources");
    }

    // This build script only generates icon assets and embeds Windows
    // resources. PDF rendering is pure Rust (junk-libs-platen) — nothing to
    // download or link.
}

fn generate_icons(workspace_root: &Path, manifest_dir: &Path) {
    use image::imageops::FilterType;

    let source = workspace_root.join("res").join("icon.png");
    let assets_dir = manifest_dir.join("assets");

    println!("cargo:rerun-if-changed={}", source.display());

    if !source.exists() {
        println!(
            "cargo:warning=Source icon not found at {}, skipping icon generation",
            source.display()
        );
        return;
    }

    // Skip if source hasn't changed (compare modification times)
    let source_mtime = fs::metadata(&source).and_then(|m| m.modified()).ok();
    let png_output = assets_dir.join("icon-256.png");
    let png_mtime = fs::metadata(&png_output).and_then(|m| m.modified()).ok();
    if let (Some(src), Some(dst)) = (source_mtime, png_mtime)
        && dst >= src
    {
        return;
    }

    println!(
        "cargo:warning=Generating icon assets from {}",
        source.display()
    );
    fs::create_dir_all(&assets_dir).expect("Failed to create assets directory");

    let img = image::open(&source).expect("Failed to load source icon");

    // macOS expects ~10% transparent margin so the system can apply its
    // automatic drop-shadow / Liquid Glass treatments and so the icon sits at
    // the same visual size as its neighbors in the Dock and ⌘-Tab switcher.
    // Windows and Linux render icons as-is, so they keep the full-bleed master.
    let padded = pad_for_macos(&img);

    // Full-bleed runtime icon — used by Linux/Windows window icons and copied
    // into the Linux AppImage as the desktop icon.
    img.resize_exact(256, 256, FilterType::Lanczos3)
        .save(&png_output)
        .expect("Failed to save icon-256.png");

    // Padded runtime icon — used by macOS at `cargo run` time so the Dock icon
    // sits at the right visual size during development.
    padded
        .resize_exact(256, 256, FilterType::Lanczos3)
        .save(assets_dir.join("icon-256-mac.png"))
        .expect("Failed to save icon-256-mac.png");

    // Windows ICO (multi-resolution: 16, 32, 48, 256) — full-bleed.
    generate_ico(&img, &assets_dir.join("icon.ico"));

    // macOS ICNS — padded.
    generate_icns(&padded, &assets_dir.join("icon.icns"));

    println!("cargo:warning=Icon assets generated successfully");
}

fn pad_for_macos(img: &image::DynamicImage) -> image::DynamicImage {
    use image::imageops::FilterType;
    use image::{Rgba, RgbaImage};

    let canvas_size = img.width().max(img.height());
    // Apple's icon grid: body fills ~824/1024 ≈ 80.5% of the canvas.
    let body_size = (f64::from(canvas_size) * 0.805).round() as u32;
    let body = img
        .resize_exact(body_size, body_size, FilterType::Lanczos3)
        .to_rgba8();
    let mut canvas = RgbaImage::from_pixel(canvas_size, canvas_size, Rgba([0, 0, 0, 0]));
    let offset = i64::from((canvas_size - body_size) / 2);
    image::imageops::overlay(&mut canvas, &body, offset, offset);
    image::DynamicImage::ImageRgba8(canvas)
}

fn generate_ico(img: &image::DynamicImage, output: &Path) {
    use image::imageops::FilterType;
    use std::io::Write;

    let sizes: &[u32] = &[16, 32, 48, 256];
    let mut png_entries: Vec<Vec<u8>> = Vec::new();

    for &size in sizes {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let mut png_data = Vec::new();
        resized
            .write_to(
                &mut std::io::Cursor::new(&mut png_data),
                image::ImageFormat::Png,
            )
            .expect("Failed to encode PNG for ICO");
        png_entries.push(png_data);
    }

    let mut ico = Vec::new();
    // ICO header: reserved (2), type=1 (2), count (2)
    ico.write_all(&[0, 0]).unwrap(); // reserved
    ico.write_all(&1u16.to_le_bytes()).unwrap(); // type: ICO
    ico.write_all(&(sizes.len() as u16).to_le_bytes()).unwrap();

    // Calculate data offset: header(6) + entries(16 each)
    let mut data_offset: u32 = 6 + (sizes.len() as u32) * 16;

    // Directory entries
    for (i, &size) in sizes.iter().enumerate() {
        let width_byte = if size >= 256 { 0u8 } else { size as u8 };
        ico.push(width_byte); // width
        ico.push(width_byte); // height
        ico.push(0); // color palette count
        ico.push(0); // reserved
        ico.write_all(&1u16.to_le_bytes()).unwrap(); // color planes
        ico.write_all(&32u16.to_le_bytes()).unwrap(); // bits per pixel
        ico.write_all(&(png_entries[i].len() as u32).to_le_bytes())
            .unwrap();
        ico.write_all(&data_offset.to_le_bytes()).unwrap();
        data_offset += png_entries[i].len() as u32;
    }

    // Image data
    for entry in &png_entries {
        ico.write_all(entry).unwrap();
    }

    fs::write(output, &ico).expect("Failed to write icon.ico");
}

fn generate_icns(img: &image::DynamicImage, output: &Path) {
    use icns::{IconFamily, IconType, Image as IcnsImage, PixelFormat};
    use image::imageops::FilterType;

    // TODO: bump the master at res/icon.png to 1024×1024 and add
    // RGBA32_256x256_2x (512) and RGBA32_512x512_2x (1024) entries for sharper
    // rendering on retina Dock and Mission Control.
    let icon_types: &[(u32, IconType)] = &[
        (16, IconType::RGBA32_16x16),
        (32, IconType::RGBA32_32x32),
        (64, IconType::RGBA32_64x64),
        (128, IconType::RGBA32_128x128),
        (256, IconType::RGBA32_256x256),
        (512, IconType::RGBA32_512x512),
    ];

    let mut family = IconFamily::new();

    for &(size, icon_type) in icon_types {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icns_image = IcnsImage::from_data(PixelFormat::RGBA, size, size, rgba.into_raw())
            .unwrap_or_else(|e| panic!("Failed to create ICNS image {size}x{size}: {e}"));
        family
            .add_icon_with_type(&icns_image, icon_type)
            .unwrap_or_else(|e| panic!("Failed to add ICNS icon {size}x{size}: {e}"));
    }

    let file = fs::File::create(output).expect("Failed to create icon.icns");
    family.write(file).expect("Failed to write icon.icns");
}
