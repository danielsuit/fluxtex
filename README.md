# FluXTeX

A native desktop LaTeX editor written in Rust, built around two pieces of
custom infrastructure:

1. **A from-scratch pure-Rust LaTeX engine
   ([aldutex](https://github.com/danielsuit/aldutex), tracked here as a
   submodule).** FluXTeX compiles through aldutex by default. An optional
   Tectonic CLI fallback handles constructs aldutex hasn't grown into yet, so
   the rendered PDF stays faithful while the engine matures. See
   [`document/compiler.rs`](crates/fluxtex-app/src/document/compiler.rs).

2. **Real-time collaborative editing on a CRDT + WebRTC stack.** A small
   signaling server ([`fluxtex-signal`](crates/fluxtex-signal)) brokers SDP
   and ICE between peers; once the WebRTC data channel is open, peers sync
   directly using [cola](https://crates.io/crates/cola-crdt) operations. The
   demo also runs a shared-file watcher in parallel as a belt-and-suspenders
   path for single-machine verification.

Plus the usual editor surface: floem-based UI, custom LaTeX syntax
highlighting against the editor's rope, multi-page PDF preview via pdfium,
and a native macOS menu bar.

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
| [`crates/fluxtex-signal`](crates/fluxtex-signal) | Signaling server (lib for the wire types, bin for the relay) |
| [`vendor/aldutex`](vendor/aldutex) | Pure-Rust LaTeX engine, as a Git submodule and the primary compiler |
| [`vendor/pdfium`](vendor/pdfium) | Pdfium headers + license; the `.dylib` is fetched out-of-band (see *Setup*) |
| [`vendor/tectonic`](vendor/tectonic) | Optional fallback compiler location |

## Setup

### Prerequisites

- Rust 1.78+
- macOS (the menu bar code uses `objc2-app-kit`; the rest is portable)
- `vendor/aldutex` initialized as a submodule (see below)

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

### Optional: install Tectonic for the fallback compiler

When aldutex declines a document, FluXTeX can hand it to Tectonic so the
preview still renders. If you don't install Tectonic, aldutex-only documents
still compile — you'll just see an error pane for anything aldutex can't
handle.

To enable the fallback, install Tectonic via Homebrew:

```bash
brew install tectonic
```

…or drop a binary at `vendor/tectonic/bin/tectonic`, or point
`FLUXTEX_TECTONIC_BIN` at one. Bundle lookup order is documented at the
[bottom of this README](#tectonic-fallback-lookup-order).

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

Click **Host** in one window, type the displayed 6-character code into the
other window's *Room code* field, and click **Join**. The status pill turns
green when the WebRTC data channel is established; edits flow either way
through the CRDT.

The demo additionally watches the shared autosave path on disk, which is
useful for sanity-checking edit propagation without involving the network
stack.

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

## Tectonic fallback lookup order

When the dispatcher falls back to Tectonic,
[`document/compiler.rs`](crates/fluxtex-app/src/document/compiler.rs) locates
the binary and optional bundle in this order:

| Resource | Sources, in priority |
|---|---|
| Binary | `FLUXTEX_TECTONIC_BIN`, `vendor/tectonic/bin/tectonic`, `vendor/tectonic/tectonic`, `tectonic` on `PATH` |
| Bundle | `FLUXTEX_TECTONIC_BUNDLE`, `vendor/tectonic/bundles/{default.zip,default.bundle,tectonic-default.bundle}`, `vendor/tectonic/default.bundle` |

If no bundle is configured, Tectonic uses its default network bundle on the
first fallback compile, then caches it.

## License

[MIT](LICENSE).
