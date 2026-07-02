mod components;
mod layout;
mod node;
mod renderer;
mod theme;

use taffy::{FlexDirection, NodeId};

use crate::components::text;
use crate::node::builder::BobTheBuilder;
use crate::theme::THEME;
use crate::{components::div, layout::AppLayout};

fn build_layout(layout: &mut AppLayout) -> (NodeId, NodeId, NodeId) {
    let header = div()
        .border_color(THEME.border)
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
        .border_color(THEME.border)
        .border_t(1.)
        .build(layout);

    (header, content, footer)
}

fn main() {
    let mut layout = AppLayout::new();

    let (_header, content, _footer) = build_layout(&mut layout);

    components::grid("calendar", 7, Some(4))
        .border_color(THEME.border)
        .parent_node(content)
        .foreach_children(|kid, i| {
            println!("Modify kid {}", kid.name);
            kid.set_layout(|l| {
                l.flex_direction = FlexDirection::Column;
            });
            let label = format!("calendar-header_{}", i).to_owned();
            kid.add_child(
                text(format!("label {}", i + 1))
                    .px(5.)
                    .py(5.)
                    .name(node::NodeName::other(label))
                    .border_b_1()
                    .border_color(THEME.border),
            );
        })
        .build(&mut layout);

    renderer::run(layout);
}
