# Rusty Calendar PI

- Goal: small calendar UI, desktop fallback, low CPU.
- Main flow: `src/main.rs` -> `src/layout.rs` -> `src/renderer/`.
- Shared UI: `src/components.rs`, `src/node/`.
- Visuals: `src/theme/`, `src/icons/`.
- Agent docs: `agents/*.md`.
- Rule: edit the owning module, keep changes small.
- Check: `cargo check`; `cargo run` for renderer/visual changes.
