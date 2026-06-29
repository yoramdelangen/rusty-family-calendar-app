use taffy::{
    NodeId,
    prelude::{auto, length, percent},
};
use tiny_skia::{Color, Point, Rect};

use crate::{
    layout::AppLayout,
    node::{Node, NodeName, State},
};

use super::{NodeKind, Style};

#[derive(Clone, Debug)]
pub struct Builder {
    name: NodeName,
    kind: NodeKind,
    style: Style,
    parent_node: Option<NodeId>,
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
    // pub fn width_auto(mut self) -> Self {
    //     self.style.layout.size.width = auto();
    //     self
    // }

    pub fn height(mut self, height: f32) -> Self {
        self.style.layout.size.height = taffy::Dimension::length(height);
        self
    }

    pub fn height_auto(mut self) -> Self {
        self.style.layout.size.height = auto();
        self
    }

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

    pub fn border_b(mut self, size: f32) -> Self {
        self.style.layout.border.bottom = length(size);
        self
    }
    pub fn border_b_1(self) -> Self {
        self.border_b(1.)
    }

    // --- CHILDREN HELPERS
    pub fn child(mut self, child: Builder) -> Self {
        self.children.push(child);
        self
    }

    pub fn build(self, mut layout: &mut AppLayout) -> taffy::NodeId {
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

    pub fn layout(mut self, f: impl FnOnce(&mut taffy::Style)) -> Self {
        f(&mut self.style.layout);
        self
    }

    pub fn style(mut self, f: impl FnOnce(&mut Style)) -> Self {
        f(&mut self.style);
        self
    }
}
