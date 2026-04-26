# FluXTeX Implementation Status

This document records what is already in place in the current workspace and what still needs to be wired up for collaborative editing to work end to end.

## What Has Been Done

- The app shell is in place with a split editor/preview layout in `crates/fluxtex-app/src/app.rs`.
- The UI shell has been upgraded to a professional three-pane workspace in `crates/fluxtex-app/src/app.rs` with toolbar, collapsible navigation and preview panels, outline and problems areas, and a status bar.
- Local LaTeX compilation is wired through `CompilerThread` and the compile result is shown in the UI.
- Local LaTeX compilation now runs through the external Tectonic CLI in `crates/fluxtex-app/src/document/compiler.rs`, with support for repo-local binaries and bundles under `vendor/tectonic`.
- PDF output is rendered to PNG with `pdfium-render` in `crates/fluxtex-app/src/document/pdf_render.rs`.
- Local CRDT tracking is present in `crates/fluxtex-app/src/document/buffer_diff.rs` using `similar` to compute text changes.
- A network message protocol exists in `crates/fluxtex-app/src/document/network.rs` with `SyncMessage` for insert and delete operations, plus bincode encode/decode helpers and remote-apply helpers.
- `cola-crdt` is configured with the `serde` feature so CRDT operations can be serialized.
- A WebRTC scaffold exists in `crates/fluxtex-app/src/document/webrtc.rs` with peer connection and data channel handler types, signaling event queues, remote description / ICE application, and message send / receive plumbing.
- Local text diffs now produce concrete outbound `SyncMessage` values instead of placeholder comments in `crates/fluxtex-app/src/document/buffer_diff.rs`.
- Remote delete integration now exists in `crates/fluxtex-app/src/document/buffer.rs`.
- A collaboration controller now exists in `crates/fluxtex-app/src/collab/mod.rs` to group transport and remote-apply logic.
- Each app instance now creates its `DocumentBuffer` with a unique replica id in `crates/fluxtex-app/src/app.rs`, avoiding peer identity collisions once sync is enabled.
- The app entry point launches the Floem UI from `crates/fluxtex-app/src/main.rs`.
- The signaling crate exists at `crates/fluxtex-signal`, and a placeholder binary currently prints a startup message.
- The app has been compiled and launched successfully in the workspace during verification.
- The app no longer depends on the Rust `tectonic` crate at build time, so `cargo check -p fluxtex-app` is no longer blocked by the `icu-uc` build dependency from that crate.

## What Still Needs To Be Put In Place

- Connect the WebRTC state to the app UI so users can start or join a session from buttons or controls in the editor.
- Replace the placeholder signaling flow with a real file-based local exchange, or another automatic signaling mechanism suitable for two local peers.
- Persist and exchange SDP/ICE data between peers so the connection can actually complete without manual copy-paste.
- Implement the signaling server in `crates/fluxtex-signal` if the project should support a relay-based mode in addition to local file signaling.
- Connect remote collaboration events back into the reactive editor state so received inserts/deletes update the visible text without being mistaken for local edits.
- Replace the current `text_input`-based document area with a richer multiline/code editor widget that can support syntax highlighting, gutter diagnostics, and command-palette-grade editing affordances.
- Add a basic two-instance test flow to confirm edits propagate in both directions.

## Current Gaps By Area

### UI

- The app now has the right shell structure, but Host/Join collaboration controls are not exposed yet.
- The problems panel currently shows human-readable compile state text, but not parsed TeX diagnostics with clickable file/line jumps.
- The editor surface still uses a basic text input rather than a full code editor with syntax coloring, minimap, gutter markers, and true distraction-free behavior.
- Incoming remote text is not yet pushed back into the Floem editor state.

### Networking

- WebRTC connection setup now exposes queued signaling events and sync message send/receive helpers, but there is still no complete signaling exchange.
- The app does not yet poll or display WebRTC signaling / connection events.

### CRDT Sync

- Local diff calculation now emits serializable sync operations.
- Remote synchronization can be applied to `DocumentBuffer`, but it is not yet wired through the visible editor state.

### Server / Relay

- The signaling crate is only a stub.
- There is no implemented relay server, persistence, or session management.

## Suggested Next Order Of Work

1. Replace the editor surface with a richer Floem editor component so the new shell can support line-aware editing and diagnostics.
2. Hook `CollaborationSession` into `app.rs` so local edits are broadcast and remote edits update the visible text state safely.
3. Wire file-based SDP and ICE exchange for two local peers using the queued `SignalingEvent`s.
4. Add Host/Join controls and connection status feedback.
5. Verify two running app instances propagate edits in both directions.

## Notes

- The current codebase is already beyond scaffolding: the editor, rendering, diffing, message type, and WebRTC types exist.
- The main remaining work is signaling exchange plus app-level UI/state wiring around the collaboration core that now exists.
