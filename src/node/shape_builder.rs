use tiny_skia::Color;

use crate::{layout::AppLayout, node::NodeName};

use super::{NodeKind, ShapeContent, ShapeKind, builder::{BobTheBuilder, Builder}};

#[derive(Debug)]
pub struct ShapeBuilder {
    builder: Builder,
}

impl ShapeBuilder {
    pub fn new(color: Color) -> Self {
        Self {
            builder: Builder::new(NodeKind::Shape(ShapeContent::new(color)), None),
        }
    }

    pub fn circle(mut self) -> Self {
        if let NodeKind::Shape(shape) = &mut self.builder.kind {
            shape.kind = ShapeKind::Circle;
        }
        self
    }

    pub fn oval(mut self) -> Self {
        if let NodeKind::Shape(shape) = &mut self.builder.kind {
            shape.kind = ShapeKind::Oval;
        }
        self
    }

    pub fn rect(mut self) -> Self {
        if let NodeKind::Shape(shape) = &mut self.builder.kind {
            shape.kind = ShapeKind::Rect;
        }
        self
    }

    pub fn rounded_rect(mut self, radius: f32) -> Self {
        if let NodeKind::Shape(shape) = &mut self.builder.kind {
            shape.kind = ShapeKind::RoundedRect(radius);
        }
        self
    }

    pub fn polygon(mut self, points: Vec<tiny_skia::Point>) -> Self {
        if let NodeKind::Shape(shape) = &mut self.builder.kind {
            shape.kind = ShapeKind::Polygon(points);
        }
        self
    }

    pub fn name(mut self, name: NodeName) -> Self {
        self.builder.name = name;
        self
    }

    pub fn parent_node(mut self, parent_node: taffy::NodeId) -> Self {
        self.builder.parent_node = Some(parent_node);
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.builder = self.builder.width(width);
        self
    }

    pub fn width_full(mut self) -> Self {
        self.builder = self.builder.width_full();
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.builder = self.builder.height(height);
        self
    }

    pub fn height_full(mut self) -> Self {
        self.builder = self.builder.height_full();
        self
    }

    pub fn layout(mut self, f: impl FnOnce(&mut taffy::Style)) -> Self {
        self.builder = self.builder.layout(f);
        self
    }

    pub fn build(self, layout: &mut AppLayout) -> taffy::NodeId {
        self.builder.build(layout)
    }
}
