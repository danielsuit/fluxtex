use floem::prelude::*;
use floem::reactive::{create_rw_signal, create_effect};
use crate::document::buffer::DocumentBuffer;
use crate::document::compiler::{CompilerThread, CompileResult};
use crate::document::pdf_render::render_pdf_to_png;
use crossbeam_channel::unbounded;
use std::sync::{Arc, Mutex};

pub fn app_view() -> impl View {
    // Basic state signals
    let doc_buffer = Arc::new(Mutex::new(DocumentBuffer::new()));
    let content_signal = create_rw_signal(String::new());
    
    // PDF View output
    let pdf_status = create_rw_signal("Ready".to_string());
    
    let rendered_image = create_rw_signal(None::<Vec<u8>>);
    
    let (tx, rx) = unbounded::<CompileResult>();
    
    let compiler = Arc::new(CompilerThread::new(tx));
    
    let compile_results = floem::ext_event::create_signal_from_channel(rx);
    
    create_effect(move |_| {
        if let Some(res) = compile_results.get() {
            match res {
                CompileResult::Ok(data) => {
                    pdf_status.set(format!("Compiled PDF successfully: {} bytes", data.len()));
                    
                    match render_pdf_to_png(&data) {
                        Ok(png_bytes) => rendered_image.set(Some(png_bytes)),
                        Err(err) => pdf_status.set(format!("Render Error: {}", err)),
                    }
                },
                CompileResult::Err(err) => {
                    pdf_status.set(format!("Compile Error: {}", err));
                }
            }
        }
    });

    let content_sig = content_signal;
    let compiler_clone = compiler.clone();
    
    v_stack((
        label(|| "FluXTeX - P2P LaTeX Editor".to_string()).style(|s| s.padding(10.0)),
        
        button(label(|| "Compile".to_string())).on_click_stop(move |_| {
            let current = content_sig.get();
            pdf_status.set("Compiling...".to_string());
            compiler_clone.compile(current);
        }),

        h_stack((
            // Editor Side
            v_stack((
                label(|| "Editor".to_string()).style(|s| s.padding(5.0)),
                text_input(content_signal)
                    .style(|s| s.width_full().height_full())
            )).style(|s| s.width_pct(50.0).height_full()),
            
            // Preview Side
            v_stack((
                label(|| "Preview".to_string()).style(|s| s.padding(5.0)),
                label(move || pdf_status.get())
                    .style(|s| s.width_full().padding(10.0)),
                
                dyn_container(
                    move || rendered_image.get(),
                    move |img_opt| {
                        if let Some(png_bytes) = img_opt {
                            img(move || png_bytes.clone()).style(|s| s.width_full().height_full()).into_any()
                        } else {
                            empty().into_any()
                        }
                    }
                ).style(|s| s.width_full().height_full())
                
            )).style(|s| s.width_pct(50.0).height_full().border_left(1.0).border_color(floem::peniko::Color::GRAY)),
        )).style(|s| s.width_full().height_full())
    )).style(|s| s.width_full().height_full())
}
