use floem::*;

mod document;
mod ui;
mod collab;
mod highlight;
mod app;

fn main() {
    tracing_subscriber::fmt::init();
    
    // Spawn tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    // Start Floem app
    floem::launch(app::app_view);
}
