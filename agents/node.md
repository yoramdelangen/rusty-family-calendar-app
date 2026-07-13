# Node System

- `mod.rs`: node state + draw code.
- `builder.rs`: general node builder. It collects name, kind, style, parent, and children, then `build()` creates one `AppLayout` node and recurses into child builders.
- `grid_builder.rs`: grid-only wrapper around `Builder` that preconfigures grid layout and injects per-cell children before the same `build()` pass.
- `shape_builder.rs`: dedicated shape builder feeding `NodeKind::Shape`.
- Shapes draw directly in `Node::draw` with tiny-skia paths for rect, rounded rect, circle, oval, and polygon.
- Keep state, layout flags, and draw logic here.
- Add helpers only if more than one caller needs them.
- Prefer the direct builder -> node flow already here; do not add extra layers unless one builder path stops being enough.
