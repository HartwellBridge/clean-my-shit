//! Renders the "Clean My Shit" icon from `icon.svg` (next to this crate) via
//! resvg, exporting:
//!   <out>/icon-1024.png   source art (macOS .icns is built from this)
//!   <out>/icon.png        256px (Linux / generic / app header)
//!   <out>/icon.ico        multi-resolution Windows icon
//!
//! Vector → crisp at every size, no external assets.

use std::path::{Path, PathBuf};

use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};

const SVG: &str = include_str!("../icon.svg");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets"));
    std::fs::create_dir_all(&out)?;

    write_png(&out.join("icon-1024.png"), 1024)?;
    write_png(&out.join("icon.png"), 256)?;
    write_ico(&out.join("icon.ico"))?;

    println!("icons written to {}", out.display());
    Ok(())
}

/// Render the SVG into a `size`×`size` pixmap.
fn render(size: u32) -> Pixmap {
    let tree = usvg::Tree::from_str(SVG, &usvg::Options::default()).expect("icon.svg failed to parse");
    let mut pixmap = Pixmap::new(size, size).expect("pixmap alloc");
    let scale = size as f32 / 1024.0;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap
}

fn write_png(path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    let png = render(size).encode_png().map_err(|e| format!("png encode: {e}"))?;
    std::fs::write(path, png)?;
    Ok(())
}

fn write_ico(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16u32, 32, 48, 64, 128, 256] {
        let pixmap = render(size);
        // tiny_skia stores premultiplied alpha; ico wants straight RGBA.
        let rgba = unpremultiply(pixmap.data());
        let img = ico::IconImage::from_rgba_data(size, size, rgba);
        dir.add_entry(ico::IconDirEntry::encode(&img)?);
    }
    dir.write(std::fs::File::create(path)?)?;
    Ok(())
}

fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for px in data.chunks_exact(4) {
        let a = px[3];
        if a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let f = 255.0 / a as f32;
            out.push((px[0] as f32 * f).round().min(255.0) as u8);
            out.push((px[1] as f32 * f).round().min(255.0) as u8);
            out.push((px[2] as f32 * f).round().min(255.0) as u8);
            out.push(a);
        }
    }
    out
}
