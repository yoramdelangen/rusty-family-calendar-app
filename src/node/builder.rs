use taffy::{
    Display, NodeId,
    prelude::{auto, length, percent},
};
use tiny_skia::Color;

use crate::{layout::AppLayout, node::NodeName};

use super::{NodeKind, Style};

pub trait BobTheBuilder {
    fn build(self, layout: &mut AppLayout) -> taffy::NodeId;
}

#[derive(Clone, Debug)]
pub struct Builder {
    pub name: NodeName,
    pub kind: NodeKind,
    pub style: Style,
    pub parent_node: Option<NodeId>,
    children: Vec<Builder>,
}

impl Builder {
    pub fn new(kind: NodeKind, parent_node: Option<NodeId>) -> Self {
        Self {
            name: NodeName::NoName,
            kind,
            style: Style::default(),
            parent_node,
            children: Vec::new(),
        }
    }

    pub fn name(mut self, name: NodeName) -> Self {
        self.name = name;
        self
    }

    pub fn set_name(&mut self, name: NodeName) {
        self.name = name;
    }

    pub fn display(mut self, display: Display) -> Self {
        self.style.layout.display = display;
        self
    }

    // --- SIZINGS: width, height
    pub fn parent_node(mut self, parent: NodeId) -> Self {
        self.parent_node = Some(parent);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.style.layout.size.width = taffy::Dimension::length(width);
        self
    }

    pub fn width_full(mut self) -> Self {
        self.style.layout.size.width = percent(1.);
        self
    }

    pub fn width_auto(mut self) -> Self {
        self.style.layout.size.width = auto();
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.style.layout.size.height = taffy::Dimension::length(height);
        self
    }

    pub fn height_full(mut self) -> Self {
        self.style.layout.size.height = percent(1.);
        self
    }

    pub fn height_auto(mut self) -> Self {
        self.style.layout.size.height = auto();
        self
    }

    // --- COLORING
    pub fn background(mut self, color: Color) -> Self {
        self.style.background_color = Some(color);
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.style.text_color = color;
        self
    }

    // --- BORDERS
    pub fn border_color(mut self, color: Color) -> Self {
        self.style.border_color = Some(color);
        self
    }

    pub fn set_border_color(&mut self, color: Color) {
        self.style.border_color = Some(color);
    }

    pub fn border_b(mut self, size: f32) -> Self {
        self.style.layout.border.bottom = length(size);
        self
    }
    pub fn border_b_1(self) -> Self {
        self.border_b(1.)
    }

    pub fn border_t(mut self, size: f32) -> Self {
        self.style.layout.border.top = length(size);
        self
    }
    pub fn border_t_1(self) -> Self {
        self.border_t(1.)
    }

    pub fn border_l(mut self, size: f32) -> Self {
        self.style.layout.border.left = length(size);
        self
    }
    pub fn border_l_1(self) -> Self {
        self.border_l(1.)
    }

    pub fn set_border_l(&mut self, size: f32) {
        self.style.layout.border.left = length(size);
    }

    pub fn set_border_l_1(&mut self) {
        self.set_border_l(1.)
    }

    pub fn border_r(mut self, size: f32) -> Self {
        self.style.layout.border.right = length(size);
        self
    }
    pub fn border_r_1(self) -> Self {
        self.border_r(1.)
    }

    // --- CHILDREN HELPERS
    pub fn child(mut self, child: Builder) -> Self {
        self.children.push(child);
        self
    }

    pub fn add_child(&mut self, child: Builder) {
        self.children.push(child);
    }

    // --- HELPER FN's
    pub fn layout(mut self, f: impl FnOnce(&mut taffy::Style)) -> Self {
        f(&mut self.style.layout);
        self
    }

    pub fn style(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.style);
        self
    }
}

impl BobTheBuilder for Builder {
    fn build(self, mut layout: &mut AppLayout) -> taffy::NodeId {
        let node_id = layout.create_node(self.name, self.kind, self.style, self.parent_node);

        let child_ids: Vec<_> = self
            .children
            .into_iter()
            .map(|kid| kid.build(&mut layout))
            .collect();

        if child_ids.len() > 0 {
            layout.add_children(node_id, &child_ids);
        }

        node_id
    }
}
