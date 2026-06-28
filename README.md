# Rusty calander PI

Goal is a lightweight GUI that requires minimal CPU usage for rendering graphics.

## Deps

- tiny-skia for drawing
- cosmic-text for future text rendering
- taffy for layout
- renderer:
  - macOS/desktop: softbuffer + winit
  - Pi target: DRM/KMS

## Linkers

```bash
rustup target add arm-unknown-linux-musleabihf
brew install arm-linux-gnueabihf-binutils
```

## Input

Input is intentionally disabled right now on both backends.

## Remote Pi Build

```bash
./build-pi-remote.sh
```

Defaults:

```bash
REMOTE=yoram@192.168.0.29
REMOTE_DIR=~/rusty-calendar-pi
TARGET=arm-unknown-linux-gnueabihf
```
