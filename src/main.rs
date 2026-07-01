mod components;
mod layout;
mod node;
mod renderer;
mod theme;

use std::cell::LazyCell;
use std::sync::Once;

use once_cell::sync::Lazy;
use taffy::NodeId;

use crate::node::builder::BobTheBuilder;
use crate::{components::div, layout::AppLayout};

fn build_layout(layout: &mut AppLayout) -> (NodeId, NodeId, NodeId) {
    let header = div()
        .border_color(layout.theme.border)
        .name(node::NodeName::Header)
        .height(38.0)
        .border_b(1.0)
        .build(layout);

    let content = div()
        .name(node::NodeName::Content)
        .width_full()
        .layout(|l| {
            l.flex_grow = 1.0;
            l.flex_shrink = 1.0;
        })
        .build(layout);

    let footer = div()
        .name(node::NodeName::Footer)
        .width_full()
        .height(38.0)
        .border_color(layout.theme.border)
        .border_t(1.)
        .build(layout);

    (header, content, footer)
}

static THEME: Lazy<theme::Theme> = Lazy::new(|| {
    let base16_light = theme::Base16 {
        base00: theme::hex(0xeff1f5),
        base01: theme::hex(0xe6e9ef),
        base02: theme::hex(0xccd0da),
        base03: theme::hex(0xbcc0cc),
        base04: theme::hex(0xacb0be),
        base05: theme::hex(0x4c4f69),
        base06: theme::hex(0xdc8a78),
        base07: theme::hex(0x7287fd),
        base08: theme::hex(0xd20f39),
        base09: theme::hex(0xfe640b),
        base0a: theme::hex(0xdf8e1d),
        base0b: theme::hex(0x40a02b),
        base0c: theme::hex(0x179299),
        base0d: theme::hex(0x1e66f5),
        base0e: theme::hex(0x8839ef),
        base0f: theme::hex(0xdd7878),
    };
    theme::Theme::from_base16(base16_light)
});

fn main() {
    let mut layout = AppLayout::new(THEME.clone());

    let (_header, content, _footer) = build_layout(&mut layout);

    let cal = components::grid("calendar", 7, Some(4))
        .border_color(layout.theme.border)
        .parent_node(content);

    cal.build(&mut layout);

    // building elements on the page

    renderer::run(layout);
}
