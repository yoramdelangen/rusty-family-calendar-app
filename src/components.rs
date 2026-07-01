use crate::node::{NodeKind, NodeName, builder::Builder, grid_builder::GridBuilder, next_node_id};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}

pub fn grid(name: &str, columns: usize, rows: Option<usize>) -> GridBuilder {
    GridBuilder::new(name, columns, rows)
}

pub fn grid_item(name: &str) -> Builder {
    Builder::new(NodeKind::GridItem, None)
        .name(NodeName::GridItem(format!("{}-{}", name, next_node_id())))
        // .width_full()
        // .background(THEME.raw.base09)
        // .border_color(THEME.raw.base08)
        // .height_full()
        // .layout(|l| {
        //     l.flex_grow = 1.;
        //     l.flex_shrink = 1.;
        // })
        .border_b_1()
}

