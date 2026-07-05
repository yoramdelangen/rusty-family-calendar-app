use taffy::Display;

use crate::{
    icons::IconInfo,
    node::{
        NodeKind, NodeName, TextContent, builder::Builder, grid_builder::GridBuilder, next_node_id,
    },
};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}

pub fn text(val: impl Into<String>) -> Builder {
    Builder::new(NodeKind::Text(TextContent::new(val)), None)
}

pub fn icon(icon: &str) -> Builder {
    Builder::new(NodeKind::Icon(IconInfo::new(icon)), None).name(NodeName::icon(icon))
}

pub fn grid(name: &str, columns: usize, rows: Option<usize>) -> GridBuilder {
    GridBuilder::new(name, columns, rows)
}

pub fn grid_item(name: &str) -> Builder {
    Builder::new(NodeKind::GridItem, None)
        .name(NodeName::GridItem(format!("{}-{}", name, next_node_id())))
        .border_b_1()
}

pub fn pill(content: impl Into<String>) -> Builder {
    text(content)
        .width_auto()
        .py(2.)
        .px(4.)
        .rounded_xl()
        .kind_meta(|kind| {
            if let NodeKind::Text(txt_content) = kind {
                txt_content.is_pill = true;
            }
        })
        // .background(THEME.surface_raised)
        .display(Display::Flex)
}
