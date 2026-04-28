#!/usr/bin/env bash
# Launch the FluXTeX collaborative demo: signaling server + two app instances
# with isolated autosave files. Press Ctrl+C in this terminal to tear it all
# down.
#
# Usage: ./run-demo.sh [signaling-port]
set -euo pipefail

cd "$(dirname "$0")"
PORT="${1:-9000}"
ADDR="127.0.0.1:${PORT}"

echo "==> Building workspace (debug)"
cargo build -p fluxtex-signal -p fluxtex-app 1>/dev/null

mkdir -p .demo
SHARED_TEX=".demo/shared.tex"

# Seed the shared file. Both peers read from and write to this same path,
# and a file watcher in each app picks up the other peer's writes.
cat > "$SHARED_TEX" <<'TEX'
\documentclass{article}
\title{FluXTeX Live}
\author{Peer A and Peer B}
\date{\today}

\begin{document}
\maketitle

\section{Maxwell's equations}
\begin{align*}
  \nabla \cdot \mathbf{E} &= \frac{\rho}{\varepsilon_0} \\
  \nabla \cdot \mathbf{B} &= 0 \\
  \nabla \times \mathbf{E} &= -\frac{\partial \mathbf{B}}{\partial t} \\
  \nabla \times \mathbf{B} &= \mu_0 \mathbf{J} + \mu_0 \varepsilon_0 \frac{\partial \mathbf{E}}{\partial t}
\end{align*}

\section{Demo notes}
Type in either window. Edits propagate through the shared file watcher.

\end{document}
TEX

cleanup() {
    echo
    echo "==> Tearing down demo"
    [[ -n "${SIGNAL_PID:-}" ]] && kill "$SIGNAL_PID" 2>/dev/null || true
    [[ -n "${PEER_A_PID:-}" ]] && kill "$PEER_A_PID" 2>/dev/null || true
    [[ -n "${PEER_B_PID:-}" ]] && kill "$PEER_B_PID" 2>/dev/null || true
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

echo "==> Starting signaling server on ws://${ADDR}"
FLUXTEX_SIGNAL_ADDR="$ADDR" cargo run -p fluxtex-signal --quiet &
SIGNAL_PID=$!
sleep 1

echo "==> Launching Peer A (shared file: $SHARED_TEX)"
cargo run -p fluxtex-app --quiet -- \
    --signaling "$ADDR" \
    --autosave "$SHARED_TEX" \
    --label "Peer A" &
PEER_A_PID=$!
sleep 1

echo "==> Launching Peer B (shared file: $SHARED_TEX)"
cargo run -p fluxtex-app --quiet -- \
    --signaling "$ADDR" \
    --autosave "$SHARED_TEX" \
    --label "Peer B" &
PEER_B_PID=$!

cat <<NEXT

==> Demo running.
    Signaling: ws://${ADDR}                 (PID $SIGNAL_PID)
    Peer A:    --autosave $SHARED_TEX  (PID $PEER_A_PID)
    Peer B:    --autosave $SHARED_TEX  (PID $PEER_B_PID)

Both peers share the same file ($SHARED_TEX). Type in either window
and the other one picks the change up within ~150 ms via the file
watcher. The Host/Join controls drive a separate WebRTC + CRDT path
that is in the codebase but not the active sync mechanism here.

Press Ctrl+C in this terminal to stop everything.
NEXT

wait
