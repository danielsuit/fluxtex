use crate::collab::{CollabEvent, CollabStatus, CollaborationSession};
use crate::document::buffer::DocumentBuffer;
use crate::document::compiler::{CompileResult, CompilerThread};
use crate::document::pdf_render::{render_pdf_to_png, RenderedPage};
use crate::highlight::{latex_styling, EditorHandle, LatexThemeColors};
use crate::macos_menu::{install_app_menu, AppMenuCommand};
use crate::CollabConfig;
use crossbeam_channel::{unbounded, RecvTimeoutError, Sender};
use floem::event::{Event, EventListener};
use floem::peniko::Color;
use floem::prelude::*;
use floem::reactive::{create_effect, create_rw_signal, SignalGet, SignalUpdate};
use floem::style::{AlignItems, CursorStyle};
use floem::text::Weight;
use floem::views::editor::core::editor::EditType;
use floem::views::editor::core::selection::Selection;
use floem::views::editor::text::RenderWhitespace;
use floem::views::text_editor::text_editor;
use floem::views::v_stack_from_iter;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

const STARTER_LATEX: &str = r#"\documentclass{article}
\title{FluxTeX Starter}
\author{FluXTeX}
\date{\today}

\begin{document}
\maketitle

\section{Introduction}
This starter document gives us something real to edit and compile.

Inline math: $E = mc^2$.

\subsection{Checklist}
\begin{itemize}
  \item Type into the editor and press Enter for new lines
  \item Compile the manuscript
  \item Review the rendered PDF preview
\end{itemize}

\[
  \int_0^1 x^2\,dx = \frac{1}{3}
\]

\end{document}
"#;
const AUTOSAVE_PATH: &str = "main.tex";
const AUTOSAVE_DEBOUNCE_MS: u64 = 350;

#[derive(Clone)]
enum AutosaveResult {
    Saved,
    Failed(String),
}

#[derive(Clone, Copy)]
struct Theme {
    id: u64,
    name: &'static str,
    background: Color,
    panel: Color,
    panel_alt: Color,
    editor: Color,
    accent: Color,
    accent_soft: Color,
    border: Color,
    border_strong: Color,
    text: Color,
    text_muted: Color,
    success: Color,
    warning: Color,
    error: Color,
    syntax_comment: Color,
    syntax_command: Color,
    syntax_environment: Color,
    syntax_math: Color,
    syntax_delimiter: Color,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ThemePreset {
    Midnight,
    Paper,
    Forest,
    Rose,
    Solarized,
}

impl ThemePreset {
    fn next(self) -> Self {
        match self {
            Self::Midnight => Self::Paper,
            Self::Paper => Self::Forest,
            Self::Forest => Self::Rose,
            Self::Rose => Self::Solarized,
            Self::Solarized => Self::Midnight,
        }
    }
}

fn palette(preset: ThemePreset) -> Theme {
    match preset {
        ThemePreset::Midnight => Theme {
            id: 1,
            name: "Midnight",
            background: Color::rgb8(13, 16, 21),
            panel: Color::rgb8(20, 24, 30),
            panel_alt: Color::rgb8(27, 32, 40),
            editor: Color::rgb8(17, 21, 27),
            accent: Color::rgb8(96, 165, 250),
            accent_soft: Color::rgba8(96, 165, 250, 38),
            border: Color::rgba8(255, 255, 255, 12),
            border_strong: Color::rgba8(255, 255, 255, 24),
            text: Color::rgb8(232, 236, 243),
            text_muted: Color::rgb8(141, 151, 170),
            success: Color::rgb8(93, 196, 141),
            warning: Color::rgb8(233, 184, 73),
            error: Color::rgb8(231, 108, 92),
            syntax_comment: Color::rgb8(112, 125, 148),
            syntax_command: Color::rgb8(114, 176, 255),
            syntax_environment: Color::rgb8(234, 191, 116),
            syntax_math: Color::rgb8(127, 220, 184),
            syntax_delimiter: Color::rgb8(198, 167, 255),
        },
        ThemePreset::Paper => Theme {
            id: 2,
            name: "Paper",
            background: Color::rgb8(246, 247, 250),
            panel: Color::rgb8(255, 255, 255),
            panel_alt: Color::rgb8(241, 244, 248),
            editor: Color::rgb8(255, 255, 255),
            accent: Color::rgb8(46, 111, 214),
            accent_soft: Color::rgba8(46, 111, 214, 22),
            border: Color::rgba8(15, 23, 42, 18),
            border_strong: Color::rgba8(15, 23, 42, 42),
            text: Color::rgb8(31, 41, 55),
            text_muted: Color::rgb8(100, 116, 139),
            success: Color::rgb8(52, 139, 89),
            warning: Color::rgb8(190, 135, 24),
            error: Color::rgb8(196, 76, 62),
            syntax_comment: Color::rgb8(120, 132, 148),
            syntax_command: Color::rgb8(46, 111, 214),
            syntax_environment: Color::rgb8(181, 119, 34),
            syntax_math: Color::rgb8(39, 142, 118),
            syntax_delimiter: Color::rgb8(112, 85, 200),
        },
        ThemePreset::Forest => Theme {
            id: 3,
            name: "Forest",
            background: Color::rgb8(17, 23, 20),
            panel: Color::rgb8(25, 33, 29),
            panel_alt: Color::rgb8(34, 43, 38),
            editor: Color::rgb8(20, 28, 24),
            accent: Color::rgb8(111, 196, 143),
            accent_soft: Color::rgba8(111, 196, 143, 34),
            border: Color::rgba8(255, 255, 255, 12),
            border_strong: Color::rgba8(255, 255, 255, 26),
            text: Color::rgb8(231, 238, 233),
            text_muted: Color::rgb8(150, 168, 156),
            success: Color::rgb8(120, 212, 162),
            warning: Color::rgb8(225, 190, 96),
            error: Color::rgb8(222, 112, 102),
            syntax_comment: Color::rgb8(110, 136, 122),
            syntax_command: Color::rgb8(124, 220, 170),
            syntax_environment: Color::rgb8(235, 198, 120),
            syntax_math: Color::rgb8(116, 204, 217),
            syntax_delimiter: Color::rgb8(186, 165, 255),
        },
        ThemePreset::Rose => Theme {
            id: 4,
            name: "Rose",
            background: Color::rgb8(24, 19, 28),
            panel: Color::rgb8(33, 27, 39),
            panel_alt: Color::rgb8(43, 36, 51),
            editor: Color::rgb8(28, 23, 34),
            accent: Color::rgb8(232, 130, 180),
            accent_soft: Color::rgba8(232, 130, 180, 34),
            border: Color::rgba8(255, 255, 255, 14),
            border_strong: Color::rgba8(255, 255, 255, 28),
            text: Color::rgb8(239, 233, 243),
            text_muted: Color::rgb8(172, 158, 180),
            success: Color::rgb8(139, 211, 183),
            warning: Color::rgb8(236, 194, 118),
            error: Color::rgb8(238, 118, 132),
            syntax_comment: Color::rgb8(136, 120, 150),
            syntax_command: Color::rgb8(255, 150, 202),
            syntax_environment: Color::rgb8(255, 206, 132),
            syntax_math: Color::rgb8(129, 219, 200),
            syntax_delimiter: Color::rgb8(196, 170, 255),
        },
        ThemePreset::Solarized => Theme {
            id: 5,
            name: "Solarized",
            background: Color::rgb8(244, 236, 214),
            panel: Color::rgb8(253, 246, 227),
            panel_alt: Color::rgb8(238, 232, 213),
            editor: Color::rgb8(253, 246, 227),
            accent: Color::rgb8(38, 139, 210),
            accent_soft: Color::rgba8(38, 139, 210, 24),
            border: Color::rgba8(88, 110, 117, 38),
            border_strong: Color::rgba8(88, 110, 117, 70),
            text: Color::rgb8(72, 90, 96),
            text_muted: Color::rgb8(123, 137, 139),
            success: Color::rgb8(133, 153, 0),
            warning: Color::rgb8(181, 137, 0),
            error: Color::rgb8(220, 50, 47),
            syntax_comment: Color::rgb8(147, 161, 161),
            syntax_command: Color::rgb8(38, 139, 210),
            syntax_environment: Color::rgb8(203, 75, 22),
            syntax_math: Color::rgb8(42, 161, 152),
            syntax_delimiter: Color::rgb8(108, 113, 196),
        },
    }
}

fn primary_button(title: &'static str, on_click: impl Fn(()) + 'static, theme: Theme) -> impl View {
    button(label(move || title.to_string()))
        .on_click_stop(move |_| on_click(()))
        .style(move |s| {
            s.padding_horiz(14.0)
                .padding_vert(7.0)
                .border(0.0)
                .border_radius(6.0)
                .background(theme.accent)
                .color(Color::WHITE)
                .font_size(12.5)
                .font_weight(Weight::SEMIBOLD)
                .hover(|s| s.background(theme.accent.multiply_alpha(0.88)))
                .active(|s| s.background(theme.accent.multiply_alpha(0.78)))
        })
}

fn ghost_button(title: &'static str, on_click: impl Fn(()) + 'static, theme: Theme) -> impl View {
    button(label(move || title.to_string()))
        .on_click_stop(move |_| on_click(()))
        .style(move |s| {
            s.padding_horiz(10.0)
                .padding_vert(6.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(theme.border)
                .background(Color::TRANSPARENT)
                .color(theme.text)
                .font_size(12.0)
                .hover(|s| {
                    s.background(theme.panel_alt)
                        .border_color(theme.border_strong)
                })
                .active(|s| s.background(theme.panel_alt))
        })
}

fn icon_button(glyph: &'static str, on_click: impl Fn(()) + 'static, theme: Theme) -> impl View {
    button(label(move || glyph.to_string()))
        .on_click_stop(move |_| on_click(()))
        .style(move |s| {
            s.width(28.0)
                .height(26.0)
                .items_center()
                .justify_center()
                .border(1.0)
                .border_radius(6.0)
                .border_color(theme.border)
                .background(Color::TRANSPARENT)
                .color(theme.text_muted)
                .font_size(13.0)
                .hover(|s| {
                    s.background(theme.panel_alt)
                        .border_color(theme.border_strong)
                        .color(theme.text)
                })
                .active(|s| s.background(theme.panel_alt))
        })
}

fn toggle_button(
    title: &'static str,
    active: impl Fn() -> bool + 'static,
    on_click: impl Fn(()) + 'static,
    theme: Theme,
) -> impl View {
    button(label(move || title.to_string()))
        .on_click_stop(move |_| on_click(()))
        .style(move |s| {
            let is_active = active();
            s.padding_horiz(10.0)
                .padding_vert(6.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(if is_active {
                    theme.accent
                } else {
                    theme.border
                })
                .background(if is_active {
                    theme.accent_soft
                } else {
                    Color::TRANSPARENT
                })
                .color(if is_active {
                    theme.text
                } else {
                    theme.text_muted
                })
                .font_size(12.0)
                .hover(|s| s.border_color(theme.border_strong))
        })
}

fn meta_chip(text: impl Fn() -> String + 'static, theme: Theme) -> impl View {
    label(text).style(move |s| {
        s.padding_horiz(8.0)
            .padding_vert(3.0)
            .border(1.0)
            .border_radius(4.0)
            .border_color(theme.border)
            .color(theme.text_muted)
            .font_size(11.0)
    })
}

fn section_header(
    title: &'static str,
    subtitle: impl Fn() -> String + 'static,
    theme: Theme,
) -> impl View {
    h_stack((
        label(move || title.to_string()).style(move |s| {
            s.font_size(11.0)
                .font_weight(Weight::SEMIBOLD)
                .color(theme.text_muted)
        }),
        label(subtitle).style(move |s| s.font_size(11.0).color(theme.text_muted)),
    ))
    .style(move |s| {
        s.justify_between()
            .items_center()
            .width_full()
            .padding_horiz(2.0)
    })
}

fn clamp_width(value: f64, min_width: f64, max_width: f64) -> f64 {
    value.max(min_width).min(max_width)
}

fn status_label(status: &CollabStatus) -> &'static str {
    match status {
        CollabStatus::Idle => "Solo",
        CollabStatus::ConnectingToSignaling => "Connecting…",
        CollabStatus::WaitingForPeer => "Waiting for peer",
        CollabStatus::Joining => "Joining room…",
        CollabStatus::Negotiating => "Negotiating…",
        CollabStatus::Connected => "Connected",
        CollabStatus::Disconnected(_) => "Disconnected",
    }
}

fn status_color(theme: Theme, status: &CollabStatus) -> Color {
    match status {
        CollabStatus::Connected => theme.success,
        CollabStatus::WaitingForPeer | CollabStatus::Joining | CollabStatus::Negotiating => {
            theme.warning
        }
        CollabStatus::ConnectingToSignaling => theme.accent,
        CollabStatus::Disconnected(_) => theme.error,
        CollabStatus::Idle => theme.text_muted,
    }
}

fn collab_session_widget(
    theme: Theme,
    status_signal: floem::reactive::RwSignal<CollabStatus>,
    code_signal: floem::reactive::RwSignal<String>,
    join_input: floem::reactive::RwSignal<String>,
    collab_state: Arc<Mutex<Option<CollaborationSession>>>,
    signaling_addr_signal: floem::reactive::RwSignal<String>,
    log: floem::reactive::RwSignal<String>,
) -> impl View {
    use floem::views::text_input;

    dyn_container(
        move || status_signal.get(),
        move |status| {
            let collab_state = collab_state.clone();
            let is_idle = matches!(status, CollabStatus::Idle | CollabStatus::Disconnected(_));

            if is_idle {
                let collab_for_host = collab_state.clone();
                let host_btn = button(label(|| "Host".to_string()))
                    .on_click_stop(move |_| {
                        let mut lock = collab_for_host.lock().unwrap();
                        if lock.is_some() {
                            return;
                        }
                        let addr = signaling_addr_signal.get();
                        match CollaborationSession::host(addr) {
                            Ok(session) => {
                                *lock = Some(session);
                                status_signal.set(CollabStatus::ConnectingToSignaling);
                            }
                            Err(e) => {
                                log.update(|l| l.push_str(&format!("host failed: {e}\n")));
                            }
                        }
                    })
                    .style(move |s| {
                        s.padding_horiz(12.0)
                            .padding_vert(6.0)
                            .border(0.0)
                            .border_radius(6.0)
                            .background(theme.accent)
                            .color(Color::WHITE)
                            .font_size(12.0)
                            .font_weight(Weight::SEMIBOLD)
                            .hover(|s| s.background(theme.accent.multiply_alpha(0.88)))
                    });

                let collab_for_join = collab_state.clone();
                let join_btn = button(label(|| "Join".to_string()))
                    .on_click_stop(move |_| {
                        let mut lock = collab_for_join.lock().unwrap();
                        if lock.is_some() {
                            return;
                        }
                        let addr = signaling_addr_signal.get();
                        let code = join_input.get().trim().to_uppercase();
                        if code.is_empty() {
                            log.update(|l| l.push_str("enter a room code first\n"));
                            return;
                        }
                        match CollaborationSession::join(addr, code) {
                            Ok(session) => {
                                *lock = Some(session);
                                status_signal.set(CollabStatus::ConnectingToSignaling);
                            }
                            Err(e) => {
                                log.update(|l| l.push_str(&format!("join failed: {e}\n")));
                            }
                        }
                    })
                    .style(move |s| {
                        s.padding_horiz(10.0)
                            .padding_vert(6.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(theme.border)
                            .background(Color::TRANSPARENT)
                            .color(theme.text)
                            .font_size(12.0)
                            .hover(|s| {
                                s.background(theme.panel_alt)
                                    .border_color(theme.border_strong)
                            })
                    });

                let code_field = text_input(join_input)
                    .placeholder("Room code")
                    .style(move |s| {
                        s.width(110.0)
                            .padding_horiz(8.0)
                            .padding_vert(5.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(theme.border)
                            .background(theme.panel_alt)
                            .color(theme.text)
                            .font_size(12.0)
                    });

                h_stack((host_btn, code_field, join_btn))
                    .style(move |s| s.gap(6.0).items_center())
                    .into_any()
            } else {
                let collab_for_leave = collab_state.clone();
                let label_text = status_label(&status).to_string();
                let pill_color = status_color(theme, &status);

                let leave_btn = button(label(|| "Leave".to_string()))
                    .on_click_stop(move |_| {
                        let mut lock = collab_for_leave.lock().unwrap();
                        if let Some(mut session) = lock.take() {
                            session.shutdown();
                        }
                        status_signal.set(CollabStatus::Idle);
                        code_signal.set(String::new());
                    })
                    .style(move |s| {
                        s.padding_horiz(10.0)
                            .padding_vert(6.0)
                            .border(1.0)
                            .border_radius(6.0)
                            .border_color(theme.border)
                            .background(Color::TRANSPARENT)
                            .color(theme.text)
                            .font_size(12.0)
                            .hover(|s| {
                                s.background(theme.panel_alt)
                                    .border_color(theme.border_strong)
                            })
                    });

                h_stack((
                    container(empty()).style(move |s| {
                        s.width(7.0)
                            .height(7.0)
                            .border_radius(999.0)
                            .background(pill_color)
                    }),
                    label(move || label_text.clone()).style(move |s| {
                        s.font_size(12.0)
                            .font_weight(Weight::SEMIBOLD)
                            .color(theme.text)
                    }),
                    label(move || {
                        let code = code_signal.get();
                        if code.is_empty() {
                            String::new()
                        } else {
                            format!("· {code}")
                        }
                    })
                    .style(move |s| {
                        s.font_size(12.0)
                            .color(theme.accent)
                            .font_weight(Weight::SEMIBOLD)
                    }),
                    leave_btn,
                ))
                .style(move |s| s.gap(8.0).items_center())
                .into_any()
            }
        },
    )
}

fn clamp_height(value: f64, min_height: f64, max_height: f64) -> f64 {
    value.max(min_height).min(max_height)
}

fn pane_resize_handle(theme: Theme, on_delta: impl Fn(f64) + 'static) -> impl View {
    let drag_origin = create_rw_signal(None::<f64>);
    let handle = container(empty());
    let handle_id = handle.id();

    handle
        .on_event_stop(EventListener::PointerDown, move |event| {
            if let Event::PointerDown(pointer) = event {
                if pointer.button.is_primary() {
                    drag_origin.set(Some(pointer.pos.x));
                    handle_id.request_active();
                }
            }
        })
        .on_event_stop(EventListener::PointerMove, move |event| {
            if let Event::PointerMove(pointer) = event {
                if let Some(previous_x) = drag_origin.get_untracked() {
                    on_delta(pointer.pos.x - previous_x);
                    drag_origin.set(Some(pointer.pos.x));
                }
            }
        })
        .on_event_stop(EventListener::PointerUp, move |_| {
            drag_origin.set(None);
            handle_id.clear_active();
        })
        .style(move |s| {
            s.width(5.0)
                .height_full()
                .cursor(CursorStyle::ColResize)
                .background(theme.border)
                .hover(|s| s.background(theme.accent))
                .active(|s| s.background(theme.accent))
        })
}

fn pane_resize_handle_vertical(theme: Theme, on_delta: impl Fn(f64) + 'static) -> impl View {
    let drag_origin = create_rw_signal(None::<f64>);
    let handle = container(empty());
    let handle_id = handle.id();

    handle
        .on_event_stop(EventListener::PointerDown, move |event| {
            if let Event::PointerDown(pointer) = event {
                if pointer.button.is_primary() {
                    drag_origin.set(Some(pointer.pos.y));
                    handle_id.request_active();
                }
            }
        })
        .on_event_stop(EventListener::PointerMove, move |event| {
            if let Event::PointerMove(pointer) = event {
                if let Some(previous_y) = drag_origin.get_untracked() {
                    on_delta(pointer.pos.y - previous_y);
                    drag_origin.set(Some(pointer.pos.y));
                }
            }
        })
        .on_event_stop(EventListener::PointerUp, move |_| {
            drag_origin.set(None);
            handle_id.clear_active();
        })
        .style(move |s| {
            s.width_full()
                .height(5.0)
                .cursor(CursorStyle::RowResize)
                .background(theme.border)
                .hover(|s| s.background(theme.accent))
                .active(|s| s.background(theme.accent))
        })
}

fn extract_outline(content: &str) -> String {
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        for (prefix, depth) in [
            ("\\section{", ""),
            ("\\subsection{", "  "),
            ("\\subsubsection{", "    "),
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                if let Some(title) = rest.split('}').next() {
                    entries.push(format!("{depth}• {title}"));
                }
            }
        }
    }

    if entries.is_empty() {
        String::new()
    } else {
        entries.join("\n")
    }
}

fn extract_project_tree(content: &str) -> String {
    let mut files = vec![
        "main.tex".to_string(),
        "sections/".to_string(),
        "figures/".to_string(),
    ];

    for line in content.lines() {
        let trimmed = line.trim();
        for command in ["\\input{", "\\include{"] {
            if let Some(rest) = trimmed.strip_prefix(command) {
                if let Some(path) = rest.split('}').next() {
                    let candidate = if path.ends_with(".tex") {
                        path.to_string()
                    } else {
                        format!("{path}.tex")
                    };
                    if !files.iter().any(|entry| entry == &candidate) {
                        files.push(candidate);
                    }
                }
            }
        }
    }

    files.join("\n")
}

fn preview_summary(status: &str) -> &'static str {
    if status.starts_with("Compile Error") || status.starts_with("Render Error") {
        "Unavailable"
    } else if status.starts_with("Compiled PDF successfully") {
        "Synced"
    } else if status == "Compiling..." {
        "In progress"
    } else {
        "Idle"
    }
}

fn preview_placeholder_message(status: &str) -> &'static str {
    if status.starts_with("Compile Error") || status.starts_with("Render Error") {
        "See Problems for details."
    } else if status == "Compiling..." {
        "Compile in progress."
    } else {
        "Compile to generate a preview."
    }
}

fn build_status_color(theme: Theme, status: &str) -> Color {
    if status.starts_with("Compile Error") || status.starts_with("Render Error") {
        theme.error
    } else if status.starts_with("Compiled PDF successfully") {
        theme.success
    } else if status == "Compiling..." {
        theme.warning
    } else {
        theme.text_muted
    }
}

fn initial_editor_content(path: &str) -> String {
    std::fs::read_to_string(path)
        .ok()
        .filter(|content| !content.trim().is_empty())
        .unwrap_or_else(|| STARTER_LATEX.to_string())
}

fn spawn_autosave_worker(
    autosave_path: String,
    last_written: Arc<Mutex<String>>,
    result_tx: Sender<AutosaveResult>,
) -> Sender<String> {
    let (tx, rx) = unbounded::<String>();

    std::thread::spawn(move || {
        while let Ok(mut latest_content) = rx.recv() {
            loop {
                match rx.recv_timeout(Duration::from_millis(AUTOSAVE_DEBOUNCE_MS)) {
                    Ok(newer_content) => latest_content = newer_content,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }

            if let Some(parent) = std::path::Path::new(&autosave_path).parent() {
                if !parent.as_os_str().is_empty() {
                    let _ = std::fs::create_dir_all(parent);
                }
            }
            let result = match std::fs::write(&autosave_path, &latest_content) {
                Ok(()) => {
                    *last_written.lock().unwrap() = latest_content.clone();
                    AutosaveResult::Saved
                }
                Err(err) => AutosaveResult::Failed(err.to_string()),
            };
            let _ = result_tx.send(result);
        }
    });

    tx
}

/// Polls the autosave file and emits the new content whenever it changes
/// from disk in a way that didn't originate locally. This is the primary
/// peer-to-peer sync mechanism: both app instances point at the same file,
/// each one's autosave write becomes the other one's external change.
fn spawn_file_watcher(
    autosave_path: String,
    last_written: Arc<Mutex<String>>,
) -> crossbeam_channel::Receiver<String> {
    let (tx, rx) = unbounded::<String>();
    std::thread::spawn(move || {
        let mut last_seen: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_millis(150));
            let Ok(current) = std::fs::read_to_string(&autosave_path) else {
                continue;
            };
            let already_local = {
                let lw = last_written.lock().unwrap();
                *lw == current
            };
            if already_local {
                last_seen = Some(current);
                continue;
            }
            if last_seen.as_deref() == Some(current.as_str()) {
                continue;
            }
            last_seen = Some(current.clone());
            if tx.send(current).is_err() {
                break;
            }
        }
    });
    rx
}

pub fn app_view(config: CollabConfig) -> impl View {
    let replica_id = ((Uuid::new_v4().as_u128() as u64) | 1).max(1);
    let doc_buffer = Arc::new(Mutex::new(DocumentBuffer::with_replica_id(replica_id)));
    let initial_content = initial_editor_content(&config.autosave_path);
    let content_signal = create_rw_signal(initial_content.clone());
    let autosave_path = config.autosave_path.clone();
    let autosave_path_for_status = config.autosave_path.clone();
    let signaling_addr = config.signaling_addr.clone();
    let signaling_addr_init = signaling_addr.clone();
    let instance_label = config.instance_label.clone();
    let collab_state: Arc<Mutex<Option<CollaborationSession>>> = Arc::new(Mutex::new(None));
    let collab_status = create_rw_signal(CollabStatus::Idle);
    let collab_room_code = create_rw_signal(String::new());
    let collab_join_input = create_rw_signal(String::new());
    let collab_log = create_rw_signal(String::new());
    let signaling_addr_signal = create_rw_signal(signaling_addr_init);
    let theme_preset = create_rw_signal(ThemePreset::Midnight);
    let show_file_tree = create_rw_signal(true);
    let show_preview = create_rw_signal(true);
    let problems_expanded = create_rw_signal(false);
    let left_panel_width = create_rw_signal(248.0);
    let preview_panel_width = create_rw_signal(520.0);
    let problems_panel_height = create_rw_signal(180.0);
    let preview_zoom = create_rw_signal(100);
    let fit_page = create_rw_signal(true);
    let compile_log = create_rw_signal("Compile output will appear here.".to_string());
    let pdf_status = create_rw_signal("Ready".to_string());
    let rendered_pages = create_rw_signal(Vec::<Rc<RenderedPage>>::new());
    let editor_handle: EditorHandle = Rc::new(RefCell::new(None));
    let autosave_status = create_rw_signal(format!("Autosave on · {autosave_path_for_status}"));
    let autosave_ok = create_rw_signal(true);

    let previous_text = Rc::new(RefCell::new(String::new()));
    let doc_buffer_clone = doc_buffer.clone();
    let last_written = Arc::new(Mutex::new(initial_content.clone()));
    let (autosave_result_tx, autosave_result_rx) = unbounded::<AutosaveResult>();
    let autosave_tx = spawn_autosave_worker(
        autosave_path.clone(),
        last_written.clone(),
        autosave_result_tx,
    );
    let autosave_results = floem::ext_event::create_signal_from_channel(autosave_result_rx);
    let _ = autosave_tx.send(initial_content);

    // File-based peer sync: any change to the autosave file that didn't
    // originate locally gets pushed into the editor as an external update.
    let file_watcher_rx = spawn_file_watcher(autosave_path.clone(), last_written.clone());
    let file_watcher_signal = floem::ext_event::create_signal_from_channel(file_watcher_rx);
    let editor_handle_for_watcher = editor_handle.clone();
    let previous_text_for_watcher = previous_text.clone();
    create_effect(move |_| {
        let Some(new_content) = file_watcher_signal.get() else {
            return;
        };
        // Update the suppression marker BEFORE touching the editor so the
        // resulting on_update callback's content_signal.set() produces a
        // zero diff in the broadcast effect — i.e. we don't echo the
        // external change back to the file.
        *previous_text_for_watcher.borrow_mut() = new_content.clone();
        content_signal.set(new_content.clone());
        let editor_ref = editor_handle_for_watcher.borrow();
        if let Some(editor) = editor_ref.as_ref() {
            let doc = editor.doc();
            let total_len = doc.text().len();
            let selection = Selection::region(0, total_len);
            doc.edit(
                &mut std::iter::once((selection, new_content.as_str())),
                EditType::Other,
            );
        }
    });

    let content_tracker_sig = content_signal;
    let collab_state_for_diff = collab_state.clone();
    let previous_text_diff = previous_text.clone();
    create_effect(move |_| {
        let new_text = content_tracker_sig.get();
        let mut prev = previous_text_diff.borrow_mut();
        if new_text != *prev {
            let mut db = doc_buffer_clone.lock().unwrap();
            let mut collab_lock = collab_state_for_diff.lock().unwrap();
            if let Some(session) = collab_lock.as_mut() {
                let _ = session.broadcast_local_edit(&mut db, &prev, &new_text);
            } else {
                let _ = crate::document::buffer_diff::apply_text_diff(&mut db, &prev, &new_text);
            }
            drop(collab_lock);
            drop(db);
            *prev = new_text.clone();
            let _ = autosave_tx.send(new_text);
        }
    });

    let autosave_path_for_effect = autosave_path_for_status.clone();
    create_effect(move |_| {
        if let Some(result) = autosave_results.get() {
            match result {
                AutosaveResult::Saved => {
                    autosave_ok.set(true);
                    autosave_status.set(format!("Autosaved · {autosave_path_for_effect}"));
                }
                AutosaveResult::Failed(err) => {
                    autosave_ok.set(false);
                    autosave_status.set(format!("Autosave failed · {err}"));
                }
            }
        }
    });

    // Collab tick + drain. A small thread fires a tick every 50ms; the effect
    // below drains pending events whenever a tick arrives, applies remote sync
    // messages to the local buffer, and pushes the resulting content into the
    // editor without re-broadcasting.
    let (collab_tick_tx, collab_tick_rx) = unbounded::<()>();
    let collab_tick_signal = floem::ext_event::create_signal_from_channel(collab_tick_rx);
    std::thread::spawn(move || loop {
        if collab_tick_tx.send(()).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    });

    let collab_state_drain = collab_state.clone();
    let doc_buffer_drain = doc_buffer.clone();
    let editor_handle_drain = editor_handle.clone();
    let previous_text_drain = previous_text.clone();
    create_effect(move |_| {
        if collab_tick_signal.get().is_none() {
            return;
        }
        let mut state_lock = collab_state_drain.lock().unwrap();
        let Some(session) = state_lock.as_mut() else {
            return;
        };
        let mut buffer_lock = doc_buffer_drain.lock().unwrap();
        let events = session.drain(&mut buffer_lock);
        let updated_content = buffer_lock.content.clone();
        drop(buffer_lock);
        drop(state_lock);

        let mut remote_applied = false;
        let mut log_entries: Vec<String> = Vec::new();

        for ev in events {
            match ev {
                CollabEvent::StatusChanged(status) => {
                    log_entries.push(format!("status: {status:?}"));
                    collab_status.set(status);
                }
                CollabEvent::RoomCode(code) => {
                    log_entries.push(format!("room created: {code}"));
                    collab_room_code.set(code);
                }
                CollabEvent::PeerJoined => {
                    log_entries.push("peer joined".to_string());
                }
                CollabEvent::PeerLeft => {
                    log_entries.push("peer left".to_string());
                    collab_room_code.set(String::new());
                }
                CollabEvent::RemoteSyncApplied => {
                    remote_applied = true;
                }
                CollabEvent::Info(msg) => {
                    log_entries.push(msg);
                }
                CollabEvent::Error(msg) => {
                    log_entries.push(format!("error: {msg}"));
                }
            }
        }

        if !log_entries.is_empty() {
            collab_log.update(|log| {
                for entry in log_entries {
                    log.push_str(&entry);
                    log.push('\n');
                }
            });
        }

        if remote_applied {
            // Suppress the local-broadcast effect: by setting previous_text to
            // the post-remote content first, the upcoming reactive
            // content_signal.set(...) sees zero diff and therefore does not
            // re-broadcast.
            *previous_text_drain.borrow_mut() = updated_content.clone();
            content_signal.set(updated_content.clone());

            let editor_ref = editor_handle_drain.borrow();
            if let Some(editor) = editor_ref.as_ref() {
                let doc = editor.doc();
                let total_len = doc.text().len();
                let selection = Selection::region(0, total_len);
                doc.edit(
                    &mut std::iter::once((selection, updated_content.as_str())),
                    EditType::Other,
                );
            }
        }
    });

    let (tx, rx) = unbounded::<CompileResult>();
    let compiler = Arc::new(CompilerThread::new(tx));
    let compile_results = floem::ext_event::create_signal_from_channel(rx);
    let (menu_tx, menu_rx) = unbounded::<AppMenuCommand>();
    install_app_menu(menu_tx);
    let menu_commands = floem::ext_event::create_signal_from_channel(menu_rx);

    let trigger_compile: Rc<dyn Fn()> = {
        let compiler = compiler.clone();
        let content_for_compile = content_signal;
        Rc::new(move || {
            let current = content_for_compile.get();
            pdf_status.set("Compiling...".to_string());
            compile_log.set(
                "Compilation queued.\n\nIn a full editor this would debounce and parse diagnostics."
                    .to_string(),
            );
            compiler.compile(current);
        })
    };
    let trigger_compile_for_menu = trigger_compile.clone();

    create_effect(move |_| {
        if let Some(res) = compile_results.get() {
            match res {
                CompileResult::Ok { pdf, engine } => {
                    let status = format!("Compiled PDF successfully: {} bytes", pdf.len());
                    pdf_status.set(status.clone());

                    match render_pdf_to_png(&pdf) {
                        Ok(pages) => {
                            let page_count = pages.len();
                            let pdf_len = pdf.len();
                            rendered_pages.set(pages.into_iter().map(Rc::new).collect());
                            compile_log.set(format!(
                                "Build finished successfully.\n\nEngine: {engine}\nOutput size: {pdf_len} bytes\nPages rendered: {page_count}\nStatus: PDF rendered and ready for preview."
                            ));
                        }
                        Err(err) => {
                            let message = format!("Render Error: {err}");
                            pdf_status.set(message.clone());
                            problems_expanded.set(true);
                            compile_log.set(format!(
                                "PDF render failed after a successful TeX build.\n\n{message}"
                            ));
                        }
                    }
                }
                CompileResult::Err(err) => {
                    let message = format!("Compile Error: {err}");
                    pdf_status.set(message.clone());
                    problems_expanded.set(true);
                    compile_log.set(format!(
                        "Compilation failed.\n\n{}\n\nA production editor would parse this into structured diagnostics.",
                        err
                    ));
                }
            }
        }
    });

    create_effect(move |_| {
        if let Some(command) = menu_commands.get() {
            match command {
                AppMenuCommand::NewStarterDocument => {
                    content_signal.set(STARTER_LATEX.to_string());
                    compile_log.set(
                        "Starter document restored.\n\nUse File → Compile to build the sample manuscript."
                            .to_string(),
                    );
                }
                AppMenuCommand::OpenFile => {
                    problems_expanded.set(true);
                    compile_log.set(
                        "Open… is not yet implemented.\n\nFor now, edit main.tex directly — the editor mirrors that file."
                            .to_string(),
                    );
                }
                AppMenuCommand::SaveFile => {
                    let current = content_signal.get();
                    match std::fs::write(AUTOSAVE_PATH, &current) {
                        Ok(()) => {
                            autosave_ok.set(true);
                            autosave_status.set(format!("Saved · {AUTOSAVE_PATH}"));
                        }
                        Err(err) => {
                            autosave_ok.set(false);
                            autosave_status.set(format!("Save failed · {err}"));
                        }
                    }
                }
                AppMenuCommand::SaveFileAs => {
                    problems_expanded.set(true);
                    compile_log.set(
                        "Save As… requires a file dialog (NSSavePanel) integration that's not wired up yet."
                            .to_string(),
                    );
                }
                AppMenuCommand::ReloadFile => match std::fs::read_to_string(AUTOSAVE_PATH) {
                    Ok(content) => {
                        content_signal.set(content);
                        autosave_status.set(format!("Reloaded · {AUTOSAVE_PATH}"));
                    }
                    Err(err) => {
                        problems_expanded.set(true);
                        compile_log.set(format!("Reload failed.\n\n{err}"));
                    }
                },
                AppMenuCommand::Compile => trigger_compile_for_menu(),
                AppMenuCommand::StopCompile => {
                    pdf_status.set("Idle".to_string());
                    compile_log.set(
                        "Stop Compile is not yet implemented.\n\nThe TeX worker will finish in the background; its result will arrive normally."
                            .to_string(),
                    );
                }
                AppMenuCommand::ToggleProjectSidebar => {
                    show_file_tree.update(|value| *value = !*value);
                }
                AppMenuCommand::TogglePreview => {
                    show_preview.update(|value| *value = !*value);
                }
                AppMenuCommand::ToggleProblems => {
                    problems_expanded.update(|value| *value = !*value);
                }
                AppMenuCommand::NextTheme => {
                    theme_preset.update(|value| *value = value.next());
                }
                AppMenuCommand::PreviousTheme => {
                    theme_preset.update(|value| {
                        // next four times == previous in a 5-cycle
                        for _ in 0..4 {
                            *value = value.next();
                        }
                    });
                }
                AppMenuCommand::PreviewZoomIn => {
                    preview_zoom.update(|value| *value = (*value + 10).min(300));
                }
                AppMenuCommand::PreviewZoomOut => {
                    preview_zoom.update(|value| *value = (*value - 10).max(50));
                }
                AppMenuCommand::PreviewResetZoom => {
                    preview_zoom.set(100);
                }
                AppMenuCommand::PreviewToggleFitPage => {
                    fit_page.update(|value| *value = !*value);
                }
                AppMenuCommand::ToggleFullscreen => {
                    problems_expanded.set(true);
                    compile_log.set(
                        "Toggle Full Screen routes through the standard window menu when implemented."
                            .to_string(),
                    );
                }
                AppMenuCommand::ShowKeyboardShortcuts => {
                    problems_expanded.set(true);
                    compile_log.set(
                        "Keyboard shortcuts\n\n  ⌘N            New from starter template\n  ⌘O            Open…\n  ⌘S            Save\n  ⇧⌘S           Save As…\n  ⇧⌘R           Reload from disk\n  ⌘B            Compile manuscript\n  ⌘.            Stop compile\n  ⌘1            Toggle project sidebar\n  ⌘2            Toggle preview\n  ⇧⌘J           Toggle problems panel\n  ⌘=            Zoom preview in\n  ⌘-            Zoom preview out\n  ⌘0            Reset preview zoom\n  ⌘9            Toggle fit page\n  ⇧⌘T           Next theme\n  ⌥⌘T           Previous theme\n  ⌃⌘F           Toggle full screen"
                            .to_string(),
                    );
                }
            }
        }
    });

    dyn_container(
        move || theme_preset.get(),
        move |preset| {
            let theme = palette(preset);

            let content_for_tree = content_signal;
            let content_for_outline = content_signal;
            let content_for_words = content_signal;
            let content_for_editor = content_signal;
            let content_for_cursor = content_signal;
            let preview_compile_action = trigger_compile.clone();
            let toolbar_compile_action = trigger_compile.clone();

            // ── Toolbar ────────────────────────────────────────────────
            let autosave_path_for_label = autosave_path_for_status.clone();
            let instance_label_for_label = instance_label.clone();
            let session_widget = collab_session_widget(
                theme,
                collab_status,
                collab_room_code,
                collab_join_input,
                collab_state.clone(),
                signaling_addr_signal,
                collab_log,
            );

            let toolbar = h_stack((
                h_stack((
                    label(|| "FluXTeX".to_string()).style(move |s| {
                        s.font_size(14.0)
                            .font_weight(Weight::BOLD)
                            .color(theme.text)
                    }),
                    container(empty()).style(move |s| {
                        s.width(1.0)
                            .height(16.0)
                            .background(theme.border_strong)
                            .margin_horiz(6.0)
                    }),
                    label(move || autosave_path_for_label.clone())
                        .style(move |s| s.font_size(12.5).color(theme.text_muted)),
                    label(move || format!("[{}]", instance_label_for_label)).style(move |s| {
                        s.font_size(11.0).color(theme.text_muted).padding_horiz(6.0)
                    }),
                ))
                .style(move |s| s.gap(6.0).items_center()),
                session_widget,
                h_stack((
                    toggle_button(
                        "Sidebar",
                        move || show_file_tree.get(),
                        move |_| show_file_tree.update(|value| *value = !*value),
                        theme,
                    ),
                    toggle_button(
                        "Preview",
                        move || show_preview.get(),
                        move |_| show_preview.update(|value| *value = !*value),
                        theme,
                    ),
                    container(empty()).style(move |s| {
                        s.width(1.0)
                            .height(18.0)
                            .background(theme.border)
                            .margin_horiz(2.0)
                    }),
                    button(label(move || format!("Theme: {}", theme.name)))
                        .on_click_stop(move |_| theme_preset.update(|value| *value = value.next()))
                        .style(move |s| {
                            s.padding_horiz(10.0)
                                .padding_vert(6.0)
                                .border(1.0)
                                .border_radius(6.0)
                                .border_color(theme.border)
                                .background(Color::TRANSPARENT)
                                .color(theme.text)
                                .font_size(12.0)
                                .hover(|s| {
                                    s.background(theme.panel_alt)
                                        .border_color(theme.border_strong)
                                })
                        }),
                    primary_button(
                        "Compile",
                        move |_| {
                            toolbar_compile_action();
                        },
                        theme,
                    ),
                ))
                .style(move |s| s.gap(8.0).items_center()),
            ))
            .style(move |s| {
                s.width_full()
                    .items_center()
                    .justify_between()
                    .padding_horiz(14.0)
                    .padding_vert(8.0)
                    .background(theme.panel)
                    .border_bottom(1.0)
                    .border_color(theme.border)
            });

            // ── Sidebar ────────────────────────────────────────────────
            let nav_panel = dyn_container(
                move || show_file_tree.get(),
                move |visible| {
                    if visible {
                        v_stack((
                            section_header("PROJECT", String::new, theme),
                            label(move || extract_project_tree(&content_for_tree.get())).style(
                                move |s| {
                                    s.width_full()
                                        .font_size(13.0)
                                        .line_height(1.7)
                                        .color(theme.text)
                                        .padding_horiz(2.0)
                                },
                            ),
                            container(empty()).style(move |s| {
                                s.width_full().height(1.0).background(theme.border)
                            }),
                            section_header(
                                "OUTLINE",
                                move || {
                                    let count = extract_outline(&content_for_outline.get())
                                        .lines()
                                        .filter(|l| !l.is_empty())
                                        .count();
                                    if count == 0 {
                                        String::new()
                                    } else {
                                        format!("{count}")
                                    }
                                },
                                theme,
                            ),
                            scroll(
                                label(move || {
                                    let outline = extract_outline(&content_for_outline.get());
                                    if outline.is_empty() {
                                        "No headings yet.".to_string()
                                    } else {
                                        outline
                                    }
                                })
                                .style(move |s| {
                                    s.width_full()
                                        .font_size(12.5)
                                        .line_height(1.75)
                                        .color(theme.text_muted)
                                        .padding_horiz(2.0)
                                }),
                            )
                            .style(move |s| {
                                s.width_full().min_height(0).flex_basis(0).flex_grow(1.0)
                            }),
                        ))
                        .style(move |s| {
                            s.height_full()
                                .width_full()
                                .padding_horiz(14.0)
                                .padding_vert(14.0)
                                .gap(10.0)
                                .background(theme.panel)
                                .border_right(1.0)
                                .border_color(theme.border)
                        })
                        .style(move |s| {
                            s.height_full()
                                .min_width(220.0)
                                .max_width(420.0)
                                .width(left_panel_width.get())
                                .flex_basis(left_panel_width.get())
                                .flex_grow(0.0)
                                .flex_shrink(0.0)
                        })
                        .into_any()
                    } else {
                        empty().into_any()
                    }
                },
            );

            let nav_handle = dyn_container(
                move || show_file_tree.get(),
                move |visible| {
                    if visible {
                        pane_resize_handle(theme, move |delta| {
                            left_panel_width.update(|width| {
                                *width = clamp_width(*width + delta, 220.0, 420.0);
                            });
                        })
                        .into_any()
                    } else {
                        empty().into_any()
                    }
                },
            );

            // ── Editor ─────────────────────────────────────────────────
            let editor_tab_bar = h_stack((
                h_stack((
                    container(empty()).style(move |s| {
                        s.width(7.0).height(7.0).border_radius(999.0).background(
                            if autosave_ok.get() {
                                theme.success
                            } else {
                                theme.warning
                            },
                        )
                    }),
                    label(|| "main.tex".to_string()).style(move |s| {
                        s.font_size(13.0)
                            .font_weight(Weight::SEMIBOLD)
                            .color(theme.text)
                    }),
                ))
                .style(move |s| s.gap(8.0).items_center()),
                h_stack((
                    meta_chip(
                        move || format!("{} words", word_count(&content_for_words.get())),
                        theme,
                    ),
                    meta_chip(
                        move || format!("{} lines", content_signal.get().lines().count().max(1)),
                        theme,
                    ),
                ))
                .style(move |s| s.gap(6.0).items_center()),
            ))
            .style(move |s| {
                s.width_full()
                    .justify_between()
                    .items_center()
                    .padding_horiz(14.0)
                    .padding_vert(7.0)
                    .background(theme.panel)
                    .border_bottom(1.0)
                    .border_color(theme.border)
            });

            let editor_panel = v_stack((
                editor_tab_bar,
                text_editor(content_for_editor.get())
                    .placeholder("Start writing your manuscript…")
                    .styling(latex_styling(
                        editor_handle.clone(),
                        theme.id,
                        LatexThemeColors {
                            comment: theme.syntax_comment,
                            command: theme.syntax_command,
                            environment: theme.syntax_environment,
                            math: theme.syntax_math,
                            delimiter: theme.syntax_delimiter,
                        },
                    ))
                    .with_editor({
                        let editor_handle = editor_handle.clone();
                        move |editor| {
                            *editor_handle.borrow_mut() = Some(editor.clone());
                        }
                    })
                    .update({
                        let editor_handle = editor_handle.clone();
                        move |update| {
                            if let Some(editor) = update.editor {
                                if editor_handle.borrow().is_none() {
                                    *editor_handle.borrow_mut() = Some(editor.clone());
                                }
                                content_signal.set(editor.doc().text().to_string());
                            }
                        }
                    })
                    .editor_style(move |s| {
                        s.gutter_left_padding(12.0)
                            .gutter_right_padding(14.0)
                            .gutter_dim_color(theme.text_muted)
                            .gutter_accent_color(theme.accent)
                            .gutter_current_color(theme.accent_soft)
                            .selection_color(theme.accent_soft)
                            .current_line_color(theme.panel_alt)
                            .indent_guide(true)
                            .indent_guide_color(theme.border)
                            .render_white_space(RenderWhitespace::Trailing)
                            .scroll_beyond_last_line(false)
                    })
                    .style(move |s| {
                        s.height_full()
                            .width_full()
                            .min_width(0)
                            .flex_basis(0)
                            .flex_grow(1.0)
                            .padding(0.0)
                            .background(theme.editor)
                    }),
            ))
            .style(move |s| {
                s.height_full()
                    .min_width(0.0)
                    .flex_basis(0.0)
                    .flex_grow(1.0)
                    .max_width_full()
                    .background(theme.panel)
            });

            // ── Preview ────────────────────────────────────────────────
            let preview_handle = dyn_container(
                move || show_preview.get(),
                move |visible| {
                    if visible {
                        pane_resize_handle(theme, move |delta| {
                            preview_panel_width.update(|width| {
                                *width = clamp_width(*width - delta, 320.0, 1200.0);
                            });
                        })
                        .into_any()
                    } else {
                        empty().into_any()
                    }
                },
            );

            let preview_panel = dyn_container(
                move || show_preview.get(),
                move |visible| {
                    let trigger_compile = preview_compile_action.clone();
                    if visible {
                        let preview_header = h_stack((
                            h_stack((
                                label(|| "Preview".to_string()).style(move |s| {
                                    s.font_size(13.0)
                                        .font_weight(Weight::SEMIBOLD)
                                        .color(theme.text)
                                }),
                                label(move || preview_summary(&pdf_status.get()).to_string())
                                    .style(move |s| {
                                        s.padding_horiz(8.0)
                                            .padding_vert(2.0)
                                            .border_radius(4.0)
                                            .background(
                                                build_status_color(theme, &pdf_status.get())
                                                    .multiply_alpha(0.18),
                                            )
                                            .color(build_status_color(theme, &pdf_status.get()))
                                            .font_size(11.0)
                                            .font_weight(Weight::SEMIBOLD)
                                    }),
                            ))
                            .style(move |s| s.gap(8.0).items_center()),
                            h_stack((
                                icon_button(
                                    "−",
                                    move |_| {
                                        preview_zoom.update(|value| *value = (*value - 10).max(60))
                                    },
                                    theme,
                                ),
                                label(move || format!("{}%", preview_zoom.get())).style(move |s| {
                                    s.min_width(36.0)
                                        .font_size(11.5)
                                        .color(theme.text_muted)
                                        .justify_center()
                                }),
                                icon_button(
                                    "+",
                                    move |_| {
                                        preview_zoom.update(|value| *value = (*value + 10).min(180))
                                    },
                                    theme,
                                ),
                                toggle_button(
                                    "Fit",
                                    move || fit_page.get(),
                                    move |_| fit_page.update(|value| *value = !*value),
                                    theme,
                                ),
                                ghost_button(
                                    "Rebuild",
                                    move |_| {
                                        trigger_compile();
                                    },
                                    theme,
                                ),
                            ))
                            .style(move |s| s.gap(6.0).items_center()),
                        ))
                        .style(move |s| {
                            s.width_full()
                                .justify_between()
                                .items_center()
                                .padding_horiz(14.0)
                                .padding_vert(7.0)
                                .background(theme.panel)
                                .border_bottom(1.0)
                                .border_color(theme.border)
                        });

                        let preview_body = scroll(dyn_container(
                            move || rendered_pages.get().len(),
                            move |page_count| {
                                if page_count > 0 {
                                    let pages = rendered_pages.get();
                                    let page_views: Vec<_> = pages
                                        .iter()
                                        .enumerate()
                                        .map(|(idx, page)| {
                                            let bytes = page.png.clone();
                                            let iw = page.width as f64;
                                            let ih = page.height as f64;
                                            v_stack((
                                                label(move || format!("Page {}", idx + 1)).style(
                                                    move |s| {
                                                        s.font_size(11.0)
                                                            .color(theme.text_muted)
                                                            .padding_vert(4.0)
                                                            .flex_shrink(0.0)
                                                    },
                                                ),
                                                img(move || bytes.clone()).style(move |s| {
                                                    let pane_w = preview_panel_width.get();
                                                    let zoom = preview_zoom.get() as f64 / 100.0;
                                                    let aspect = ih / iw;
                                                    let base_w = if fit_page.get() {
                                                        (pane_w - 56.0).max(160.0)
                                                    } else {
                                                        iw
                                                    };
                                                    let display_w = (base_w * zoom).max(120.0);
                                                    let display_h = display_w * aspect;
                                                    s.width(display_w)
                                                        .height(display_h)
                                                        .flex_shrink(0.0)
                                                        .background(Color::WHITE)
                                                        .border(1.0)
                                                        .border_color(theme.border)
                                                }),
                                            ))
                                            .style(
                                                move |s| s.items_center().gap(2.0).flex_shrink(0.0),
                                            )
                                        })
                                        .collect();

                                    v_stack_from_iter(page_views)
                                        .style(move |s| {
                                            s.items_center()
                                                .padding(20.0)
                                                .gap(20.0)
                                                .flex_shrink(0.0)
                                        })
                                        .into_any()
                                } else {
                                    v_stack((
                                        label(|| "PDF".to_string()).style(move |s| {
                                            s.font_size(28.0)
                                                .font_weight(Weight::BOLD)
                                                .color(theme.text_muted)
                                                .padding_horiz(14.0)
                                                .padding_vert(8.0)
                                                .border(1.0)
                                                .border_radius(8.0)
                                                .border_color(theme.border)
                                        }),
                                        label(|| "No preview yet".to_string()).style(move |s| {
                                            s.font_size(14.0)
                                                .font_weight(Weight::SEMIBOLD)
                                                .color(theme.text)
                                        }),
                                        label(move || {
                                            preview_placeholder_message(&pdf_status.get())
                                                .to_string()
                                        })
                                        .style(move |s| s.font_size(12.0).color(theme.text_muted)),
                                    ))
                                    .style(move |s| {
                                        s.items_center().justify_center().gap(10.0).padding(40.0)
                                    })
                                    .into_any()
                                }
                            },
                        ))
                        .scroll_style(|s| s.shrink_to_fit())
                        .style(move |s| {
                            s.width_full()
                                .height_full()
                                .min_height(0)
                                .flex_basis(0)
                                .flex_grow(1.0)
                                .background(theme.background)
                        });

                        v_stack((preview_header, preview_body))
                            .style(move |s| {
                                s.height_full()
                                    .width_full()
                                    .background(theme.panel)
                                    .border_left(1.0)
                                    .border_color(theme.border)
                            })
                            .style(move |s| {
                                s.height_full()
                                    .min_width(320.0)
                                    .max_width(1200.0)
                                    .width(preview_panel_width.get())
                                    .flex_basis(preview_panel_width.get())
                                    .flex_grow(0.0)
                                    .flex_shrink(0.0)
                            })
                            .into_any()
                    } else {
                        empty().into_any()
                    }
                },
            );

            // ── Workspace ──────────────────────────────────────────────
            let workspace = h_stack((
                nav_panel,
                nav_handle,
                editor_panel,
                preview_handle,
                preview_panel,
            ))
            .style(move |s| {
                s.width_full()
                    .height_full()
                    .max_width_full()
                    .min_height(0)
                    .flex_basis(0)
                    .flex_grow(1.0)
                    .align_items(AlignItems::Stretch)
                    .background(theme.background)
            });

            // ── Problems dock ──────────────────────────────────────────
            let problems_panel = v_stack((
                button(
                    h_stack((
                        h_stack((
                            label(move || {
                                if problems_expanded.get() {
                                    "▾".to_string()
                                } else {
                                    "▸".to_string()
                                }
                            })
                            .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                            label(|| "Problems".to_string()).style(move |s| {
                                s.font_size(12.0)
                                    .font_weight(Weight::SEMIBOLD)
                                    .color(theme.text)
                            }),
                            label(move || {
                                let status = pdf_status.get();
                                if status.starts_with("Compile Error")
                                    || status.starts_with("Render Error")
                                {
                                    "1".to_string()
                                } else {
                                    "0".to_string()
                                }
                            })
                            .style(move |s| {
                                let status = pdf_status.get();
                                let color = if status.starts_with("Compile Error")
                                    || status.starts_with("Render Error")
                                {
                                    theme.error
                                } else {
                                    theme.text_muted
                                };
                                s.padding_horiz(7.0)
                                    .padding_vert(1.0)
                                    .border_radius(4.0)
                                    .background(color.multiply_alpha(0.18))
                                    .color(color)
                                    .font_size(11.0)
                                    .font_weight(Weight::SEMIBOLD)
                            }),
                        ))
                        .style(move |s| s.items_center().gap(8.0)),
                        label(move || {
                            if problems_expanded.get() {
                                "Click to collapse".to_string()
                            } else {
                                "Click to expand".to_string()
                            }
                        })
                        .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                    ))
                    .style(move |s| s.width_full().justify_between().items_center()),
                )
                .on_click_stop(move |_| problems_expanded.update(|value| *value = !*value))
                .style(move |s| {
                    s.width_full()
                        .padding_horiz(14.0)
                        .padding_vert(7.0)
                        .background(theme.panel)
                        .border(0.0)
                        .border_top(1.0)
                        .border_color(theme.border)
                        .hover(|s| s.background(theme.panel_alt))
                }),
                dyn_container(
                    move || problems_expanded.get(),
                    move |expanded| {
                        if expanded {
                            v_stack((
                                pane_resize_handle_vertical(theme, move |delta| {
                                    problems_panel_height.update(|height| {
                                        *height = clamp_height(*height - delta, 120.0, 480.0);
                                    });
                                }),
                                scroll(label(move || compile_log.get()).style(move |s| {
                                    s.width_full()
                                        .font_family("JetBrains Mono".to_string())
                                        .font_size(12.0)
                                        .line_height(1.65)
                                        .color(theme.text)
                                        .padding_horiz(14.0)
                                        .padding_vert(10.0)
                                }))
                                .style(move |s| {
                                    s.width_full()
                                        .height(problems_panel_height.get())
                                        .background(theme.panel_alt)
                                        .border_top(1.0)
                                        .border_color(theme.border)
                                }),
                            ))
                            .style(move |s| s.width_full())
                            .into_any()
                        } else {
                            empty().into_any()
                        }
                    },
                ),
            ))
            .style(move |s| s.width_full());

            // ── Status bar ─────────────────────────────────────────────
            let status_bar = h_stack((
                h_stack((
                    container(empty()).style(move |s| {
                        s.width(7.0)
                            .height(7.0)
                            .border_radius(999.0)
                            .background(build_status_color(theme, &pdf_status.get()))
                    }),
                    label(move || format!("Preview: {}", preview_summary(&pdf_status.get())))
                        .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                    label(|| "•".to_string())
                        .style(move |s| s.font_size(11.0).color(theme.border_strong)),
                    label(move || autosave_status.get()).style(move |s| {
                        s.font_size(11.0).color(if autosave_ok.get() {
                            theme.text_muted
                        } else {
                            theme.warning
                        })
                    }),
                ))
                .style(move |s| s.gap(10.0).items_center()),
                h_stack((
                    label(move || format!("{} words", word_count(&content_signal.get())))
                        .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                    label(|| "•".to_string())
                        .style(move |s| s.font_size(11.0).color(theme.border_strong)),
                    label(move || {
                        let lines = content_signal.get().lines().count().max(1);
                        format!("{lines} lines")
                    })
                    .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                    label(|| "•".to_string())
                        .style(move |s| s.font_size(11.0).color(theme.border_strong)),
                    label(move || {
                        let current = content_for_cursor.get();
                        let col = current.lines().last().unwrap_or("").chars().count() + 1;
                        format!("Ln {}, Col {}", current.lines().count().max(1), col)
                    })
                    .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                    label(|| "•".to_string())
                        .style(move |s| s.font_size(11.0).color(theme.border_strong)),
                    label(move || theme.name.to_string())
                        .style(move |s| s.font_size(11.0).color(theme.text_muted)),
                ))
                .style(move |s| s.gap(10.0).items_center()),
            ))
            .style(move |s| {
                s.width_full()
                    .justify_between()
                    .items_center()
                    .padding_horiz(14.0)
                    .padding_vert(6.0)
                    .background(theme.panel)
                    .border_top(1.0)
                    .border_color(theme.border)
            });

            v_stack((toolbar, workspace, problems_panel, status_bar)).style(move |s| {
                s.width_full()
                    .height_full()
                    .min_width_full()
                    .min_height_full()
                    .max_width_full()
                    .max_height_full()
                    .background(theme.background)
                    .color(theme.text)
                    .font_family("Inter".to_string())
            })
        },
    )
    .style(|s| {
        s.width_full()
            .height_full()
            .min_width_full()
            .min_height_full()
            .max_width_full()
            .max_height_full()
    })
}

fn word_count(content: &str) -> usize {
    content.split_whitespace().count()
}
