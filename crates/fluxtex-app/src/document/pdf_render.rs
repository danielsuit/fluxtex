use image::{ImageBuffer, RgbaImage};
use pdfium_render::prelude::*;
use std::io::Cursor;
use std::path::PathBuf;

fn locate_pdfium_dylib() -> Option<PathBuf> {
    let lib_name = Pdfium::pdfium_platform_library_name();

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Ok(exe) = std::env::current_exe() {
        // Packaged app: <exe>/../Frameworks/libpdfium.dylib
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join(&lib_name));
            candidates.push(exe_dir.join("../Frameworks").join(&lib_name));
        }
        // Dev: walk up from target/debug or target/release to repo root.
        let mut cur = exe.parent().map(|p| p.to_path_buf());
        for _ in 0..6 {
            if let Some(dir) = cur {
                candidates.push(dir.join("vendor/pdfium/lib").join(&lib_name));
                cur = dir.parent().map(|p| p.to_path_buf());
            } else {
                break;
            }
        }
    }

    candidates.into_iter().find(|p| p.exists())
}

pub struct RenderedPage {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn render_pdf_to_png(pdf_bytes: &[u8]) -> Result<Vec<RenderedPage>, String> {
    let bindings = match locate_pdfium_dylib() {
        Some(path) => Pdfium::bind_to_library(&path)
            .map_err(|e| format!("Failed to bind pdfium at {}: {}", path.display(), e))?,
        None => {
            Pdfium::bind_to_system_library().map_err(|e| format!("Failed to bind pdfium: {}", e))?
        }
    };
    let pdfium = Pdfium::new(bindings);

    let document = pdfium
        .load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;

    let pages = document.pages();
    if pages.is_empty() {
        return Err("PDF has no pages".to_string());
    }

    let render_config = PdfRenderConfig::new()
        .set_target_width(1400)
        .set_maximum_height(1800);

    let mut rendered = Vec::with_capacity(pages.len() as usize);
    for index in 0..pages.len() {
        let page = pages
            .get(index)
            .map_err(|e| format!("Failed to get page {index}: {e}"))?;

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("Failed to render page {index}: {e}"))?;

        let width = bitmap.width() as u32;
        let height = bitmap.height() as u32;

        let mut bytes = bitmap.as_raw_bytes().to_vec();
        for pixel in bytes.chunks_exact_mut(4) {
            if pixel.len() == 4 {
                let b = pixel[0];
                let r = pixel[2];
                pixel[0] = r;
                pixel[2] = b;
            }
        }

        let rgba: RgbaImage = ImageBuffer::from_raw(width, height, bytes)
            .ok_or_else(|| format!("Failed to create ImageBuffer for page {index}"))?;

        let mut buf = Cursor::new(Vec::new());
        rgba.write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| format!("Failed to encode PNG for page {index}: {e}"))?;

        rendered.push(RenderedPage {
            png: buf.into_inner(),
            width,
            height,
        });
    }

    Ok(rendered)
}
