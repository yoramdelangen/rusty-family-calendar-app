# Renderers

- `mod.rs`: backend selection.
- `winit.rs`: desktop renderer.
- `drm.rs`: Pi DRM renderer.
- Keep platform code isolated.
- Verify with `cargo run` on desktop; `cargo check` for DRM changes.
