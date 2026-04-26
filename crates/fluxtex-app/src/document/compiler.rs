use crossbeam_channel::{unbounded, Receiver, Sender};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use uuid::Uuid;

pub struct CompilerThread {
    tx: Sender<CompileRequest>,
}

pub enum CompileRequest {
    Compile(String),
}

#[derive(Clone)]
pub enum CompileResult {
    Ok(Vec<u8>),
    Err(String),
}

impl CompilerThread {
    pub fn new(result_tx: Sender<CompileResult>) -> Self {
        let (tx, rx): (Sender<CompileRequest>, Receiver<CompileRequest>) = unbounded();

        thread::spawn(move || {
            for req in rx {
                match req {
                    CompileRequest::Compile(source) => match compile_with_tectonic_cli(&source) {
                        Ok(pdf_data) => {
                            let _ = result_tx.send(CompileResult::Ok(pdf_data));
                        }
                        Err(e) => {
                            let _ = result_tx.send(CompileResult::Err(e.to_string()));
                        }
                    },
                }
            }
        });

        Self { tx }
    }

    pub fn compile(&self, content: String) {
        let _ = self.tx.send(CompileRequest::Compile(content));
    }
}

const TECTONIC_BIN_ENV: &str = "FLUXTEX_TECTONIC_BIN";
const TECTONIC_BUNDLE_ENV: &str = "FLUXTEX_TECTONIC_BUNDLE";

struct TempBuildDir(PathBuf);

impl TempBuildDir {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("fluxtex-tectonic-{}", Uuid::new_v4()));
        fs::create_dir_all(&path)
            .map_err(|err| format!("Failed to create temp build directory: {err}"))?;
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempBuildDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compile_with_tectonic_cli(source: &str) -> Result<Vec<u8>, String> {
    let build_dir = TempBuildDir::new()?;
    let input_path = build_dir.path().join("main.tex");
    let output_dir = build_dir.path().join("out");

    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("Failed to create Tectonic output directory: {err}"))?;
    fs::write(&input_path, source)
        .map_err(|err| format!("Failed to write TeX source for compilation: {err}"))?;

    let mut command =
        Command::new(find_tectonic_binary().unwrap_or_else(|| PathBuf::from("tectonic")));
    command
        .arg("-X")
        .arg("compile")
        .arg("--keep-logs")
        .arg("--keep-intermediates")
        .arg("--outdir")
        .arg(&output_dir)
        .arg(&input_path)
        .current_dir(build_dir.path());

    if let Some(bundle_path) = find_tectonic_bundle() {
        command.arg("--bundle").arg(bundle_path);
    }

    let output = command.output().map_err(format_launch_error)?;
    if !output.status.success() {
        return Err(format_failed_run(&output, &output_dir));
    }

    let pdf_path = output_dir.join("main.pdf");
    fs::read(&pdf_path).map_err(|err| {
        let diagnostics = collect_output_text(&output, &output_dir);
        format!(
            "Tectonic finished but `{}` was missing: {err}\n\n{}",
            pdf_path.display(),
            diagnostics
        )
    })
}

fn find_tectonic_binary() -> Option<PathBuf> {
    configured_path(TECTONIC_BIN_ENV).or_else(|| {
        vendor_candidates(&["vendor/tectonic/bin/tectonic", "vendor/tectonic/tectonic"])
    })
}

fn find_tectonic_bundle() -> Option<PathBuf> {
    configured_path(TECTONIC_BUNDLE_ENV).or_else(|| {
        vendor_candidates(&[
            "vendor/tectonic/bundles/default.zip",
            "vendor/tectonic/bundles/default.bundle",
            "vendor/tectonic/bundles/tectonic-default.bundle",
            "vendor/tectonic/default.bundle",
        ])
    })
}

fn configured_path(env_name: &str) -> Option<PathBuf> {
    std::env::var_os(env_name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn vendor_candidates(relative_paths: &[&str]) -> Option<PathBuf> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())?;

    relative_paths
        .iter()
        .map(|relative| repo_root.join(relative))
        .find(|candidate| candidate.exists())
}

fn format_launch_error(err: std::io::Error) -> String {
    if err.kind() == ErrorKind::NotFound {
        format!(
            "Tectonic CLI was not found.\n\nPlace a binary at `vendor/tectonic/bin/tectonic`, or set `{}` to a Tectonic executable path.",
            TECTONIC_BIN_ENV
        )
    } else {
        format!("Failed to launch Tectonic CLI: {err}")
    }
}

fn format_failed_run(output: &Output, output_dir: &Path) -> String {
    let status = output
        .status
        .code()
        .map(|code| format!("exit code {code}"))
        .unwrap_or_else(|| "terminated by signal".to_string());

    format!(
        "Tectonic compilation failed ({status}).\n\n{}",
        collect_output_text(output, output_dir)
    )
}

fn collect_output_text(output: &Output, output_dir: &Path) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let log_text = fs::read_to_string(output_dir.join("main.log"))
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    let mut sections = Vec::new();

    if !stdout.is_empty() {
        sections.push(format!("stdout:\n{stdout}"));
    }
    if !stderr.is_empty() {
        sections.push(format!("stderr:\n{stderr}"));
    }
    if let Some(log_text) = log_text {
        sections.push(format!("main.log:\n{log_text}"));
    }

    if sections.is_empty() {
        let bundle_hint = if find_tectonic_bundle().is_none() {
            format!(
                "\n\nTip: add a bundle at `vendor/tectonic/bundles/default.bundle` or set `{}`.",
                TECTONIC_BUNDLE_ENV
            )
        } else {
            String::new()
        };

        format!("Tectonic did not produce any diagnostic output.{bundle_hint}")
    } else {
        sections.join("\n\n")
    }
}
