use pdfium_render::prelude::*;
use image::{ImageBuffer, RgbaImage};
use std::io::Cursor;

pub fn render_pdf_to_png(pdf_bytes: &[u8]) -> Result<Vec<u8>, String> {
    // Bind to the default system-provided pdfium library
    let pdfium = Pdfium::new(
        Pdfium::bind_to_system_library()
            .map_err(|e| format!("Failed to bind pdfium: {}", e))?,
    );

    let document = pdfium.load_pdf_from_byte_slice(pdf_bytes, None)
        .map_err(|e| format!("Failed to load PDF: {}", e))?;

    let pages = document.pages();
    if pages.is_empty() {
         return Err("PDF has no pages".to_string());
    }
    
    // Render the first page for now
    let page = pages.get(0).map_err(|e| format!("Failed to get page 0: {}", e))?;
    
    // We can specify a scale factor for high DPI.
    let render_config = PdfRenderConfig::new()
        .set_target_width(1200) // render at decent resolution
        .set_maximum_height(1600);
        
    let bitmap = page.render_with_config(&render_config)
        .map_err(|e| format!("Failed to render page: {}", e))?;
        
    let width = bitmap.width() as u32;
    let height = bitmap.height() as u32;
    
    let mut bytes = bitmap.as_raw_bytes().to_vec();
    
    // Pdfium internally uses BGRA format. Convert to RGBA.
    for pixel in bytes.chunks_exact_mut(4) {
        if pixel.len() == 4 {
            let b = pixel[0];
            let r = pixel[2];
            pixel[0] = r;
            pixel[2] = b;
            // pixel[3] is alpha, pixel[1] is green.
        }
    }
    
    let rgba: RgbaImage = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| "Failed to create ImageBuffer".to_string())?;

    let mut buf = Cursor::new(Vec::new());
    rgba.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;
        
    Ok(buf.into_inner())
}
