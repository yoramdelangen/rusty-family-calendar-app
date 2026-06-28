#!/usr/bin/env bash
set -euo pipefail

TARGET="arm-unknown-linux-gnueabihf"

cargo build --release --target "$TARGET"
scp "target/$TARGET/release/rusty-calendar-pi" yoram@192.168.0.29:~/
