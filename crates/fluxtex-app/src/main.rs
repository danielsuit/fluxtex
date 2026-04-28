mod app;
mod collab;
mod document;
mod highlight;
mod macos_menu;

/// Per-instance configuration assembled from CLI flags so two instances on the
/// same machine can run side-by-side without clobbering each other's autosave.
#[derive(Clone)]
pub struct CollabConfig {
    pub signaling_addr: String,
    pub autosave_path: String,
    pub instance_label: String,
}

impl CollabConfig {
    fn from_args() -> Self {
        let mut signaling_addr = "127.0.0.1:9000".to_string();
        let mut autosave_path = "main.tex".to_string();
        let mut instance_label = "main".to_string();

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--signaling" | "-s" => {
                    if let Some(v) = args.next() {
                        signaling_addr = v;
                    }
                }
                "--autosave" | "-a" => {
                    if let Some(v) = args.next() {
                        autosave_path = v;
                    }
                }
                "--label" | "-l" => {
                    if let Some(v) = args.next() {
                        instance_label = v;
                    }
                }
                "--help" | "-h" => {
                    eprintln!(
                        "Usage: fluxtex-app [--signaling host:port] [--autosave path] [--label name]"
                    );
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        Self {
            signaling_addr,
            autosave_path,
            instance_label,
        }
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    let config = CollabConfig::from_args();

    // Spawn tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    let _guard = rt.enter();

    // Start Floem app
    floem::launch(move || app::app_view(config));
}
