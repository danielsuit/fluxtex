mod app;
mod collab;
mod document;
mod highlight;
mod macos_menu;
mod ui;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    // Spawn tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    // Start Floem app
    floem::launch(app::app_view);
}
