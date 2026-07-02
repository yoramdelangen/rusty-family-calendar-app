use crate::node::{NodeKind, NodeName, builder::Builder, grid_builder::GridBuilder, next_node_id};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}

pub fn text(val: impl Into<String>) -> Builder {
    Builder::new(NodeKind::Text(val.into()), None)
}

pub fn grid(name: &str, columns: usize, rows: Option<usize>) -> GridBuilder {
    GridBuilder::new(name, columns, rows)
}

pub fn grid_item(name: &str) -> Builder {
    Builder::new(NodeKind::GridItem, None)
        .name(NodeName::GridItem(format!("{}-{}", name, next_node_id())))
        .border_b_1()
}

