#!/bin/sh
# Launch the live pipeline debug viewer (temporary i3 binding).
cd /home/yogi/qhints-rs || exit 1
BIN=target/release/examples/preview
if [ ! -x "$BIN" ]; then
    cargo build --release --example preview || exit 1
fi
exec "$BIN"