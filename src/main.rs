mod renderer;

use std::collections::HashMap;

use taffy::{
    Dimension, FlexDirection, NodeId, Size, Style, TaffyTree,
    prelude::{auto, length, percent},
};
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

#[derive(Clone, Copy)]
struct NodeVisual {
    background: Color,
    border_top: Option<(f32, Color)>,
    border_bottom: Option<(f32, Color)>,
}

pub(crate) struct AppLayout {
    tree: TaffyTree<()>,
    root: NodeId,
    nodes: Vec<NodeId>,
    visuals: HashMap<NodeId, NodeVisual>,
}

impl AppLayout {
    fn new() -> Self {
        let mut tree = TaffyTree::new();

        let root = tree
            .new_leaf(Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            })
            .expect("failed to create root");

        let header = tree
            .new_leaf(Style {
                size: Size {
                    width: percent(1.0),
                    height: length(48.0),
                },
                ..Default::default()
            })
            .expect("failed to create header");
        let content = tree
            .new_leaf(Style {
                size: Size {
                    width: percent(1.0),
                    height: auto(),
                },
                flex_grow: 1.0,
                flex_shrink: 1.0,
                ..Default::default()
            })
            .expect("failed to create content");
        let footer = tree
            .new_leaf(Style {
                size: Size {
                    width: percent(1.0),
                    height: length(48.0),
                },
                ..Default::default()
            })
            .expect("failed to create footer");

        tree.set_children(root, &[header, content, footer])
            .expect("failed to attach children");

        let visuals = HashMap::from([
            (
                header,
                NodeVisual {
                    background: Color::from_rgba8(0, 0, 255, 100),
                    border_top: None,
                    border_bottom: Some((5.0, Color::from_rgba8(255, 0, 0, 100))),
                },
            ),
            (
                content,
                NodeVisual {
                    background: Color::from_rgba8(0, 255, 0, 100),
                    border_top: None,
                    border_bottom: None,
                },
            ),
            (
                footer,
                NodeVisual {
                    background: Color::from_rgba8(245, 40, 145, 100),
                    border_top: Some((1.0, Color::from_rgba8(255, 255, 255, 150))),
                    border_bottom: None,
                },
            ),
        ]);

        Self {
            tree,
            root,
            nodes: vec![header, content, footer],
            visuals,
        }
    }

    pub(crate) fn render_layout(&mut self, size: Size<Dimension>) {
        self.tree
            .set_style(
                self.root,
                Style {
                    size,
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
            )
            .expect("failed to update root style");

        self.tree
            .compute_layout(self.root, Size::max_content())
            .expect("failed to compute layout");
    }

    pub(crate) fn draw(&mut self, buffer: &mut [u32], width: u32, height: u32) {
        let mut pixmap = Pixmap::new(width, height).expect("failed to create pixmap");
        pixmap.fill(Color::from_rgba8(16, 16, 24, 255));

        for node_id in &self.nodes {
            let Some(visual) = self.visuals.get(node_id).copied() else {
                continue;
            };
            let layout = self.tree.layout(*node_id).expect("missing node layout");
            let rect = Rect::from_xywh(
                layout.location.x,
                layout.location.y,
                layout.size.width,
                layout.size.height,
            )
            .expect("invalid node rectangle");

            let mut paint = Paint::default();
            paint.set_color(visual.background);
            pixmap.fill_rect(rect, &paint, Transform::identity(), None);

            if let Some((thickness, color)) = visual.border_top {
                let mut border = Paint::default();
                border.set_color(color);
                let top = Rect::from_xywh(rect.x(), rect.y(), rect.width(), thickness)
                    .expect("invalid top border");
                pixmap.fill_rect(top, &border, Transform::identity(), None);
            }

            if let Some((thickness, color)) = visual.border_bottom {
                let mut border = Paint::default();
                border.set_color(color);
                let bottom = Rect::from_xywh(
                    rect.x(),
                    rect.y() + rect.height() - thickness,
                    rect.width(),
                    thickness,
                )
                .expect("invalid bottom border");
                pixmap.fill_rect(bottom, &border, Transform::identity(), None);
            }
        }

        let src = pixmap.data();
        for (dst, rgba) in buffer.iter_mut().zip(src.chunks_exact(4)) {
            let r = rgba[0] as u32;
            let g = rgba[1] as u32;
            let b = rgba[2] as u32;
            *dst = (r << 16) | (g << 8) | b;
        }
    }
}

fn main() {
    let layout = AppLayout::new();
    renderer::run(layout);
}
