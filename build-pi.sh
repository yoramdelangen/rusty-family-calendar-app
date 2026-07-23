#!/usr/bin/env bash
set -euo pipefail

TARGET="arm-unknown-linux-gnueabihf"
REMOTE="yoram@192.168.0.29"
FONT="assets/fonts/zed-mono-light.ttf"
REMOTE_TMP_DIR="/tmp/rusty-calendar-pi-install"

cargo build --release --target "$TARGET"

ssh "$REMOTE" "rm -rf $REMOTE_TMP_DIR && mkdir -p $REMOTE_TMP_DIR"

scp "target/$TARGET/release/rusty-calendar-pi" "$REMOTE:$REMOTE_TMP_DIR/rusty-calendar-pi"
scp "install-pi.sh" "$REMOTE:$REMOTE_TMP_DIR/"
scp "rusty-calendar-pi.service" "$REMOTE:$REMOTE_TMP_DIR/"
scp "rusty-calendar-pi.env" "$REMOTE:$REMOTE_TMP_DIR/"
scp "config.toml.example" "$REMOTE:$REMOTE_TMP_DIR/"

if [ -f "$FONT" ]; then
    scp "$FONT" "$REMOTE:$REMOTE_TMP_DIR/zed-mono-light.ttf"
fi

ssh "$REMOTE" "chmod +x $REMOTE_TMP_DIR/install-pi.sh && sudo $REMOTE_TMP_DIR/install-pi.sh $REMOTE_TMP_DIR && rm -rf $REMOTE_TMP_DIR"
