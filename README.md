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

## Theme

- https://crates.io/crates/tinty

## Icons

- https://github.com/carbon-design-system/carbon/tree/main/packages/icons/src/svg


## Alternatives

- [`embedded-graphics`](https://docs.rs/embedded-graphics/latest/embedded_graphics/examples/index.html#draw-a-rectangle-with-rounded-corners)

