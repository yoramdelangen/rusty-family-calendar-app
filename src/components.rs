use taffy::{prelude::{length, percent}, AlignItems, Display};
use tiny_skia::{Color, Point};

use crate::{
    icons::IconInfo,
    node::{
        NodeKind, NodeName, TextContent, builder::Builder, grid_builder::GridBuilder,
        next_node_id, shape_builder::ShapeBuilder,
    },
    theme::THEME,
};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}

pub fn shape(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color)
}

pub fn circle(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).circle()
}

pub fn oval(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).oval()
}

pub fn rect(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).rect()
}

pub fn rounded_rect(color: Color, radius: f32) -> ShapeBuilder {
    ShapeBuilder::new(color).rounded_rect(radius)
}

pub fn polygon(color: Color, points: Vec<Point>) -> ShapeBuilder {
    ShapeBuilder::new(color).polygon(points)
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
        .px(4.)
        .py(4.)
        .display(Display::Flex)
        .flex_dir_column()
        .layout(|l| {
            l.max_size.width = percent(1.);
            l.min_size.width = length(0.);
            l.align_items = Some(AlignItems::FlexStart);
        })
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
        .background(THEME.surface_raised)
        .display(Display::Flex)
}
