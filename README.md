# FluXTeX

A native desktop LaTeX editor written in Rust, with two interesting bets in
its architecture:

1. **A from-scratch pure-Rust LaTeX engine ([aldutex](https://github.com/danielsuit/aldutex),
   tracked here as a submodule) with a Tectonic CLI fallback.** The compile
   pipeline runs aldutex first; if aldutex returns warnings about constructs
   it would silently drop (`\maketitle`, math operators, environments it
   doesn't yet implement), the dispatcher falls back to Tectonic so the
   rendered PDF actually matches the source. As aldutex grows, the fallback
   fires less often without any code change in this repo. See
   [`document/compiler.rs`](crates/fluxtex-app/src/document/compiler.rs).

2. **Optional collaborative editing on a CRDT + WebRTC stack.** A small
   signaling server ([`fluxtex-signal`](crates/fluxtex-signal)) brokers SDP
   and ICE between peers; once the WebRTC data channel is open, peers sync
   directly using [cola](https://crates.io/crates/cola-crdt) operations. The
   demo currently ships a simpler shared-file sync as the active path so the
   recorded video runs reliably on a single machine; the WebRTC + CRDT code
   is wired up via the toolbar's Host/Join controls.

Plus the usual editor surface: floem-based UI, custom LaTeX syntax
highlighting against the editor's rope, multi-page PDF preview via pdfium,
native macOS menu bar.

## Architecture

```text
                   ┌──────────────────┐
                   │  fluxtex-signal  │  WebSocket relay; rooms keyed by
                   │  (rooms + relay) │  6-char codes; forwards SDP/ICE.
                   └─────┬────────┬───┘
                         │        │
                  WS     │        │     WS
                         ▼        ▼
                   ┌──────────┐  ┌──────────┐
                   │  Peer A  │  │  Peer B  │
                   │ fluxtex- │  │ fluxtex- │
                   │   app    │  │   app    │
                   └────┬─────┘  └────┬─────┘
                        └────────────┘
                       WebRTC DataChannel
                  (cola CRDT ops, snapshots)

    Each peer:
      ┌──────────────┐    ┌────────────────────┐    ┌─────────────────┐
      │ Floem editor │ ── │ DocumentBuffer     │ ── │ Compile pipe:   │
      │  (rope-based)│    │  (cola Replica +   │    │  aldutex first, │
      │              │    │   String mirror)   │    │  tectonic on    │
      └──────────────┘    └────────────────────┘    │  fallback.      │
                                                    └────────┬────────┘
                                                             ▼
                                                   ┌─────────────────┐
                                                   │ pdfium-render → │
                                                   │ multi-page PNG  │
                                                   │ preview         │
                                                   └─────────────────┘
```

## Layout

| Path | What's there |
|---|---|
| [`crates/fluxtex-app`](crates/fluxtex-app) | Editor binary: floem UI, compile pipeline, syntax highlighting, PDF preview, collaboration glue, macOS menu bar |
| [`crates/fluxtex-signal`](crates/fluxtex-signal) | Signaling server (lib for the wire types + bin for the relay) |
| [`vendor/aldutex`](vendor/aldutex) | Pure-Rust LaTeX engine, as a Git submodule |
| [`vendor/pdfium`](vendor/pdfium) | Pdfium headers + license; the `.dylib` is fetched out-of-band (see *Setup* below) |
| [`vendor/tectonic`](vendor/tectonic) | Empty placeholders; fetch a Tectonic CLI binary in here, or use one on `PATH` |

## Setup

### Prerequisites

- Rust 1.78+
- macOS (the menu bar code uses `objc2-app-kit`; the rest is portable)
- `tectonic` CLI somewhere reachable — either install via Homebrew
  (`brew install tectonic`), or drop a binary at `vendor/tectonic/bin/tectonic`,
  or set `FLUXTEX_TECTONIC_BIN`.

### Clone with the submodule

```bash
git clone --recurse-submodules https://github.com/danielsuit/fluxtex
cd fluxtex
```

If you've already cloned without `--recurse-submodules`:

```bash
git submodule update --init --recursive
```

### Fetch the pdfium dylib

The Pdfium headers are vendored; the dylib isn't (it's ~7 MB and platform-
specific). Fetch the macOS arm64 build:

```bash
curl -L -o /tmp/pdfium-mac-arm64.tgz \
  https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7802/pdfium-mac-arm64.tgz
tar -xzf /tmp/pdfium-mac-arm64.tgz -C vendor/pdfium
```

[`document/pdf_render.rs`](crates/fluxtex-app/src/document/pdf_render.rs)
walks up from the executable looking for `vendor/pdfium/lib/libpdfium.dylib`,
falling back to `bind_to_system_library`.

### Build

```bash
cargo build
```

## Running

The editor takes per-instance flags so two copies can run side-by-side:

```bash
cargo run -p fluxtex-app -- \
    --signaling 127.0.0.1:9000 \
    --autosave path/to/doc.tex \
    --label "My Peer"
```

### Demo: two peers + signaling server

A scripted launcher boots the signaling server and two app instances pointed
at the same shared file:

```bash
./run-demo.sh
```

Click **Host** in one window. Type the displayed 6-character code into the
other window's *Room code* field and click **Join**. The status pill turns
green when the WebRTC connection is established. Edits flow either way.

The demo also runs a file watcher on the shared autosave path, so changes
propagate even when WebRTC isn't engaged — useful for quick verification
without touching the network stack.

### Iterating on the LaTeX engine

`vendor/aldutex` is a Git submodule. To pull the latest revision, rebuild,
and verify it still drives the editor cleanly:

```bash
./update-aldutex.sh
```

## Tests

```bash
cargo test -p fluxtex-app -p fluxtex-signal
```

Covers the wire-format round-trip for `SyncMessage` (Insert/Delete/Snapshot),
the snapshot-seed-the-joiner round-trip end-to-end through the CRDT, and the
shape of the signaling JSON protocol.

## Lints

```bash
cargo fmt -p fluxtex-app -p fluxtex-signal -- --check
cargo clippy -p fluxtex-app -p fluxtex-signal --all-targets --no-deps -- -D warnings
```

(`--no-deps` skips clippy on the `aldutex` submodule, which has its own lint
budget I don't try to enforce here.)

## Tectonic lookup order

When the dispatcher falls back to Tectonic, [`document/compiler.rs`](crates/fluxtex-app/src/document/compiler.rs)
locates the binary and (optional) bundle in this order:

| Resource | Sources, in priority |
|---|---|
| Binary | `FLUXTEX_TECTONIC_BIN`, `vendor/tectonic/bin/tectonic`, `vendor/tectonic/tectonic`, `tectonic` on `PATH` |
| Bundle | `FLUXTEX_TECTONIC_BUNDLE`, `vendor/tectonic/bundles/{default.zip,default.bundle,tectonic-default.bundle}`, `vendor/tectonic/default.bundle` |

If no bundle is configured, Tectonic uses its default network bundle on the
first compile, then caches it.

## License

[MIT](LICENSE).
