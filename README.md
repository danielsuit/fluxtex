# FluXTeX
FluXTeX is a native P2P collaborative LaTeX editor written in Rust. There is NO central server for document storage. Peers discover each other via a lightweight WebRTC signaling relay, then communicate directly and sync via CRDTs.

## Architecture

```text
+----------+                     +------------------+                    +----------+
|          |    WebRTC SDP/ICE   |                  |   WebRTC SDP/ICE   |          |
|  Peer A  +-------------------->| Signaling Server |<-------------------+  Peer B  |
|          |    (WebSocket)      |  (fluxtex-signal)|     (WebSocket)    |          |
+----+-----+                     +------------------+                    +----+-----+
     |                                                                        |
     |                                                                        |
     |                           Direct P2P Connection                        |
     +------------------------------------------------------------------------+
                       WebRTC DataChannel (col-crdt Ops)
```

## Prerequisites
- Rust 1.78+
- [libdatachannel](https://github.com/paullouisageneau/libdatachannel) (e.g. `brew install libdatachannel` or `apt install libdatachannel-dev`)
- A Tectonic CLI binary, either on `PATH` or vendored at `vendor/tectonic/bin/tectonic`
- Optional: a vendored Tectonic bundle at `vendor/tectonic/bundles/default.zip`
- [pdfium](https://pdfium.googlesource.com/pdfium/) shared library installed for `pdfium-render`

## Build
```bash
cargo build --release
```

## Running
Start the minimal signaling server in one terminal:
```bash
cargo run --bin fluxtex-signal
```
Start the UI app for a peer:
```bash
cargo run --bin fluxtex-app
```

## Tectonic lookup

FluXTeX now shells out to the Tectonic CLI rather than linking the Rust `tectonic` crate directly. This avoids the ICU build dependency that was blocking `cargo check` on macOS.

The app looks for Tectonic in this order:

1. `FLUXTEX_TECTONIC_BIN`
2. `vendor/tectonic/bin/tectonic`
3. `vendor/tectonic/tectonic`
4. `tectonic` on `PATH`

If a bundle is present, it looks here:

1. `FLUXTEX_TECTONIC_BUNDLE`
2. `vendor/tectonic/bundles/default.zip`
3. `vendor/tectonic/bundles/default.bundle`
4. `vendor/tectonic/bundles/tectonic-default.bundle`
5. `vendor/tectonic/default.bundle`

## Collaboration
1. **Host**: Click **New Session**. The room ID will be displayed. Share this short alphanumeric code.
2. **Join**: Click **Join Session**. Enter the room code and the signaling server URL (default `ws://localhost:9001`). 
3. Edit collaboratively!
