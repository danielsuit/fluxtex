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

## Collaboration
1. **Host**: Click **New Session**. The room ID will be displayed. Share this short alphanumeric code.
2. **Join**: Click **Join Session**. Enter the room code and the signaling server URL (default `ws://localhost:9001`). 
3. Edit collaboratively!
