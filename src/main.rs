mod components;
mod layout;
mod node;
mod renderer;

use std::collections::HashMap;

use taffy::{
    NodeId, Size, Style,
    prelude::{auto, length, percent},
};
use tiny_skia::Color;

use crate::{components::div, layout::AppLayout};

#[derive(Clone, Copy)]
struct NodeVisual {
    background: Color,
    border_top: Option<(f32, Color)>,
    border_bottom: Option<(f32, Color)>,
}

fn build_layout(layout: &mut AppLayout) -> (NodeId, NodeId, NodeId) {
    let header = div()
        .border_color(Color::from_rgba8(255, 0, 0, 100))
        .name(node::NodeName::Header)
        .height(48.0)
        .border_b_1()
        .build(layout);

    let content = div()
        .name(node::NodeName::Content)
        .width_full()
        .layout(|l| {
            l.flex_grow = 1.0;
            l.flex_shrink = 1.0;
        })
        .background(Color::from_rgba8(0, 255, 0, 100))
        .build(layout);

    let footer = div()
        .name(node::NodeName::Footer)
        .width_full()
        .height(38.0)
        .build(layout);

    (header, content, footer)
}

fn main() {
    let mut layout = AppLayout::new();

    build_layout(&mut layout);

    // building elements on the page

    renderer::run(layout);
}
