use std::collections::HashMap;

use taffy::{FlexDirection, NodeId, TaffyTree, prelude::*};
use tiny_skia::{Color, Point};

use crate::{
    node::{Node, NodeKind, NodeName, Style},
    theme::{THEME, font::FONT},
};

type TaffyTreeHouse = TaffyTree<super::node::NodeName>;

pub(crate) struct AppLayout {
    tree: TaffyTreeHouse,
    root_node: NodeId,
    nodes: HashMap<NodeId, NodeName>,
    nodes_state: HashMap<NodeName, Node>,
}

impl AppLayout {
    pub fn new() -> Self {
        let mut tree = TaffyTree::new();

        let root = tree
            .new_leaf_with_context(
                taffy::Style {
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                NodeName::Root,
            )
            .expect("failed to create root");

        Self {
            tree,
            root_node: root,
            nodes: HashMap::new(),
            nodes_state: HashMap::new(),
        }
    }

    pub fn create_node(
        &mut self,
        name: NodeName,
        kind: NodeKind,
        style: Style,
        parent_node: Option<NodeId>,
    ) -> NodeId {
        let node_id = self
            .tree
            .new_leaf_with_context(style.layout.clone(), name.clone())
            .expect("failed creating leaf");

        self.tree
            .add_child(
                if let Some(parent_node_id) = parent_node {
                    parent_node_id
                } else {
                    self.root_node
                },
                node_id,
            )
            .expect("Cannot add child to parent");

        let node = Node::new(node_id, name, kind, style);

        // warning if the node-name already exists
        if self.nodes_state.contains_key(&node.name) {
            println!(
                "WARN: there is already a node with name {} in the list",
                node.name
            );
        }

        self.nodes.insert(node_id, node.name.clone());
        self.nodes_state.insert(node.name.clone(), node);

        node_id
    }

    // pub fn add(&mut self, node: Node) -> NodeId {
    //     let node_id = node.taffy_id;
    //
    //     self.nodes.insert(node_id, node.name.clone());
    //     self.nodes_state.insert(node.name.clone(), node);
    //     self.tree
    //         .add_child(self.root_node, node_id)
    //         .expect("failed adding child to root");
    //
    //     node_id
    // }

    pub fn add_children(&mut self, parent: NodeId, children: &[NodeId]) {
        self.tree
            .set_children(parent, children)
            .expect("failed setting children");
    }

    pub fn render_layout(&mut self, size: Size<Dimension>) {
        if self
            .tree
            .dirty(self.root_node)
            .expect("failed dirty lookup")
        {
            let root = self
                .tree
                .style(self.root_node)
                .expect("missing root style")
                .clone();

            self.tree
                .set_style(
                    self.root_node,
                    taffy::Style {
                        flex_direction: root.flex_direction,
                        flex_grow: root.flex_grow,
                        flex_shrink: root.flex_shrink,
                        size,
                        ..Default::default()
                    },
                )
                .expect("failed updating root style");
        }

        self.prepare_layout_leafs(self.root_node);

        self.tree
            .compute_layout_with_measure(
                self.root_node,
                Size::MAX_CONTENT,
                |known_dimensions, available_space, node_id, node_context, style| {
                    let Some(node_name) = node_context else {
                        unreachable!();
                    };

                    let node = self
                        .nodes_state
                        .get(&node_name)
                        .expect(format!("Cannot measure NodeName {}", node_name).as_str());

                    calculate_layout_measurement(
                        known_dimensions,
                        available_space,
                        node_id,
                        node,
                        node_name,
                        style,
                    )
                },
            )
            .expect("failed computing layout");

        // self.tree
        //     .compute_layout(self.root_node, Size::max_content())
        //     .expect("failed computing layout");

        self.compute_layout_nodes(self.root_node, Point::zero());
        // self.tree.print_tree(self.root_node);
    }

    // Prepare the layout before rendering and re-calculating the layout.
    // It walksthrough all nodes and check if something is updated.
    fn prepare_layout_leafs(&mut self, id: NodeId) {
        let children = self
            .tree
            .children(id)
            .expect("failed child lookup")
            .to_vec();

        for node_id in children {
            let node_name = self.nodes.get(&node_id).expect("missing node name").clone();
            let node_state = self
                .nodes_state
                .get_mut(&node_name)
                .expect("missing node state while computing layout");

            if node_state.dirty_layout || self.tree.dirty(node_id).expect("dirty lookup failed") {
                self.tree
                    .set_style(node_id, node_state.style.layout.clone())
                    .expect("failed updating leaf style");
            }

            self.prepare_layout_leafs(node_id);
        }
    }

    fn compute_layout_nodes(&mut self, id: NodeId, offset: Point) {
        let children = self
            .tree
            .children(id)
            .expect("failed child lookup")
            .to_vec();

        for node_id in children {
            let node_name = self.nodes.get(&node_id).expect("missing node name").clone();
            let node_state = self
                .nodes_state
                .get_mut(&node_name)
                .expect("missing node state while computing layout");

            // if let NodeName::GridItem(name) = &node_state.name {
            //     println!(
            //         "GridItem={} Offset={:?} Dirty = {} Has Pixmap = {}",
            //         name,
            //         node_state.offset,
            //         node_state.dirty_layout,
            //         node_state.pixmap.is_some()
            //     );
            // }

            if node_state.dirty_layout || self.tree.dirty(node_id).expect("dirty lookup failed") {
                let node = self.tree.layout(node_id).expect("missing layout node");

                let offset_x = offset.x + node.location.x + node.padding.left;
                let offset_y = offset.y + node.location.y + node.padding.top;

                node_state.offset = Point {
                    x: offset_x,
                    y: offset_y,
                };
                node_state.rect = tiny_skia::Rect::from_xywh(
                    offset_x,
                    offset_y,
                    node.size.width,
                    node.size.height,
                )
                .expect("incorrect measurements and offsets");

                node_state.dirty_layout = false;
                node_state.dirty_screen = true;
            }

            let offset = node_state.offset.clone();
            self.compute_layout_nodes(node_id, offset);
            // self.tree.print_tree(self.root_node);
        }
    }

    pub(crate) fn draw(&mut self, buffer: &mut [u32], window_width: u32, window_height: u32) {
        buffer.fill(self.color_to_pixel(THEME.surface));

        for node in self.nodes_state.values_mut() {
            if !node.state.visible {
                continue;
            }

            if node.dirty_screen || node.pixmap.is_none() {
                node.draw();
            }

            let Some(pixmap) = &node.pixmap else {
                continue;
            };

            let src_w = pixmap.width();
            let src_h = pixmap.height();
            let dst_y = node.rect.y() as u32;
            let dst_x = node.rect.x() as u32;
            let src = pixmap.data();

            for row in 0..src_h {
                let screen_y = dst_y + row;
                if screen_y >= window_height {
                    break;
                }

                for col in 0..src_w {
                    let screen_x = dst_x + col;
                    if screen_x >= window_width {
                        break;
                    }

                    let src_i = ((row * src_w + col) * 4) as usize;
                    let dst_i = (screen_y * window_width + screen_x) as usize;

                    let r = src[src_i] as u32;
                    let g = src[src_i + 1] as u32;
                    let b = src[src_i + 2] as u32;
                    let a = src[src_i + 3] as u32;

                    if a == 0 {
                        continue;
                    }

                    buffer[dst_i] = (r << 16) | (g << 8) | b;
                }
            }
        }
    }

    fn color_to_pixel(&self, color: Color) -> u32 {
        let c = color.to_color_u8();

        let r = c.red() as u32;
        let g = c.green() as u32;
        let b = c.blue() as u32;
        let _a = c.alpha() as u32;

        // TODO: fix alpha coloring
        (r << 16) | (g << 8) | b
    }
}

fn calculate_layout_measurement(
    known: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    _node_id: NodeId,
    node: &Node,
    _node_name: &mut NodeName,
    _style: &taffy::Style,
) -> Size<f32> {
    match &node.kind {
        NodeKind::Text(content) => {
            // println!("============");
            // println!("Known = {:?}", known);
            // println!("Content = {:?}", content);
            // println!("AvailableSpace = {:?}", available_space);
            // println!("Node = {:?}", node.style);

            // get max available with
            let max_width = match available_space.width {
                AvailableSpace::Definite(w) => Some(w),
                _ => None,
            };

            FONT.measure_text(&content, &node.style.font_size, max_width)
        }
        _ => Size {
            width: known.width.unwrap_or(0.0),
            height: known.height.unwrap_or(0.0),
        },
    }
}
