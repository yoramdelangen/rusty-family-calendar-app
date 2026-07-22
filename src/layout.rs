use std::collections::HashMap;

use chrono::Local;
use taffy::{FlexDirection, NodeId, TaffyTree, prelude::*};
use tiny_skia::{Color, Point};
use tracing::{debug, info_span, trace};

use crate::{
    event::AppEvent,
    icons::IconInfo,
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
        events: crate::node::NodeEvents,
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

        let node = Node::new(node_id, name, kind, style, events);

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

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Tick => self.update_clock(),
            AppEvent::PointerDown { x, y } => {
                debug!(x, y, "pointer down");
                self.update_pointer_state(x, y, EventDispatch::Press);
                self.dispatch_press_at(x, y);
            }
            AppEvent::PointerUp { x, y } => {
                debug!(x, y, "pointer up");
                self.update_pointer_state(x, y, EventDispatch::Release);
                self.dispatch_release_at(x, y);
            }
            AppEvent::PointerClick { x, y } => {
                debug!(x, y, "pointer click");
                self.dispatch_click_at(x, y);
            }
            AppEvent::PointerMove { x, y } => {
                self.update_pointer_state(x, y, EventDispatch::Hover);
                self.dispatch_hover_at(x, y);
            }
        }
    }

    fn update_clock(&mut self) {
        let Some(node) = self.nodes_state.get_mut(&NodeName::Clock) else {
            return;
        };

        if let NodeKind::Text(txt) = &mut node.kind {
            txt.content = Local::now().format("%H:%M:%S").to_string();
            node.dirty_layout = true;
            node.dirty_screen = true;
        }
    }

    pub fn set_text(&mut self, node_id: NodeId, text: impl Into<String>) {
        let Some(node_name) = self.nodes.get(&node_id).cloned() else {
            return;
        };

        let Some(node) = self.nodes_state.get_mut(&node_name) else {
            return;
        };

        if let NodeKind::Text(txt) = &mut node.kind {
            txt.content = text.into();
            node.dirty_layout = true;
            node.dirty_screen = true;
        }
    }

    pub fn set_text_by_name(&mut self, node_name: NodeName, text: impl Into<String>) {
        let Some(node) = self.nodes_state.get_mut(&node_name) else {
            return;
        };

        if let NodeKind::Text(txt) = &mut node.kind {
            txt.content = text.into();
            node.dirty_layout = true;
            node.dirty_screen = true;
        }
    }

    pub fn set_text_color_by_name(&mut self, node_name: NodeName, color: Color) {
        let Some(node) = self.nodes_state.get_mut(&node_name) else {
            return;
        };

        node.style.text_color = color;
        node.dirty_screen = true;
    }

    pub fn set_icon_by_name(&mut self, node_name: NodeName, icon: &str) {
        let Some(node) = self.nodes_state.get_mut(&node_name) else {
            return;
        };

        if matches!(node.kind, NodeKind::Icon(_)) {
            node.kind = NodeKind::Icon(IconInfo::new(icon));
            node.dirty_layout = true;
            node.dirty_screen = true;
        }
    }

    pub fn dispatch_click_at(&mut self, x: f32, y: f32) {
        if let Some(node_id) = self.hit_test_event(x, y, EventDispatch::Click) {
            self.dispatch_node_event(node_id, EventDispatch::Click);
        }
    }

    pub fn dispatch_press_at(&mut self, x: f32, y: f32) {
        if let Some(node_id) = self.hit_test_event(x, y, EventDispatch::Press) {
            self.dispatch_node_event(node_id, EventDispatch::Press);
        }
    }

    pub fn dispatch_release_at(&mut self, x: f32, y: f32) {
        if let Some(node_id) = self.hit_test_event(x, y, EventDispatch::Release) {
            self.dispatch_node_event(node_id, EventDispatch::Release);
        }
    }

    pub fn dispatch_hover_at(&mut self, x: f32, y: f32) {
        if let Some(node_id) = self.hit_test_event(x, y, EventDispatch::Hover) {
            self.dispatch_node_event(node_id, EventDispatch::Hover);
        }
    }

    fn hit_test_event(&self, x: f32, y: f32, event: EventDispatch) -> Option<NodeId> {
        self.nodes_state
            .values()
            .filter(|node| {
                node_has_handler(node, event)
                    && node.state.visible
                    && x >= node.rect.x()
                    && x <= node.rect.x() + node.rect.width()
                    && y >= node.rect.y()
                    && y <= node.rect.y() + node.rect.height()
            })
            .min_by(|a, b| node_area(a).total_cmp(&node_area(b)))
            .map(|node| node.taffy_id)
    }

    fn hit_test_state(&self, x: f32, y: f32, event: EventDispatch) -> Option<NodeId> {
        self.nodes_state
            .values()
            .filter(|node| {
                node_accepts_event(node, event)
                    && node.state.visible
                    && x >= node.rect.x()
                    && x <= node.rect.x() + node.rect.width()
                    && y >= node.rect.y()
                    && y <= node.rect.y() + node.rect.height()
            })
            .min_by(|a, b| node_area(a).total_cmp(&node_area(b)))
            .map(|node| node.taffy_id)
    }

    fn update_pointer_state(&mut self, x: f32, y: f32, event: EventDispatch) {
        let target = self.hit_test_state(x, y, event);

        for node in self.nodes_state.values_mut() {
            let is_target = Some(node.taffy_id) == target;
            let changed = match event {
                EventDispatch::Hover => set_bool(&mut node.state.hovered, is_target),
                EventDispatch::Press => {
                    set_bool(&mut node.state.pressed, is_target)
                        | set_bool(&mut node.state.focused, is_target)
                }
                EventDispatch::Release => set_bool(&mut node.state.pressed, false),
                EventDispatch::Click => false,
            };

            if changed {
                node.dirty_screen = true;
            }
        }
    }

    fn dispatch_node_event(&mut self, node_id: NodeId, event: EventDispatch) {
        let Some(node_name) = self.nodes.get(&node_id).cloned() else {
            return;
        };

        let handler = {
            let Some(node) = self.nodes_state.get(&node_name) else {
                return;
            };

            match event {
                EventDispatch::Click
                    if node.events.caps.contains(crate::node::EventCaps::CLICK) =>
                {
                    node.events.on_click.clone()
                }
                EventDispatch::Press
                    if node.events.caps.contains(crate::node::EventCaps::PRESS) =>
                {
                    node.events.on_press.clone()
                }
                EventDispatch::Release
                    if node.events.caps.contains(crate::node::EventCaps::RELEASE) =>
                {
                    node.events.on_release.clone()
                }
                EventDispatch::Hover
                    if node.events.caps.contains(crate::node::EventCaps::HOVER) =>
                {
                    node.events.on_hover.clone()
                }
                _ => None,
            }
        };

        if let Some(handler) = handler {
            (handler.borrow_mut())(self, node_id);
        }
    }

    pub fn render_layout(&mut self, size: Size<Dimension>) {
        debug!(width = ?size.width, height = ?size.height, "render layout");
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

        let _span = info_span!("layout_pass", width = ?size.width, height = ?size.height).entered();
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
                        .get_mut(&node_name)
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
        trace!("prepare layout leafs");
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
        trace!(node = ?id, offset_x = offset.x, offset_y = offset.y, "compute layout nodes");
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
                let node = self.tree.layout(node_id).expect("missing layout node");

                let offset_x = offset.x + node.location.x;
                let offset_y = offset.y + node.location.y;

                node_state.offset = Point {
                    x: offset_x,
                    y: offset_y,
                };

                // we basically need to make pills display: inline-block
                let target_size = if matches!(
                    &node_state.kind,
                    NodeKind::Text(txt_content) if txt_content.is_pill
                ) {
                    Size {
                        width: node.content_size.width + node.padding.horizontal_axis_sum(),
                        height: node.content_size.height + node.padding.vertical_axis_sum(),
                    }
                } else {
                    node.size
                };

                node_state.rect = tiny_skia::Rect::from_xywh(
                    offset_x,
                    offset_y,
                    target_size.width,
                    target_size.height,
                )
                .expect("incorrect measurements and offsets");

                debug!(
                    node = %node_name,
                    x = offset_x,
                    y = offset_y,
                    width = target_size.width,
                    height = target_size.height,
                    "layout rect"
                );

                node_state.dirty_layout = false;
                node_state.dirty_screen = true;
            }

            let offset = node_state.offset.clone();
            self.compute_layout_nodes(node_id, offset);
            // self.tree.print_tree(self.root_node);
        }
    }

    pub(crate) fn draw(&mut self, buffer: &mut [u32], window_width: u32, window_height: u32) {
        debug!(
            window_width,
            window_height,
            buffer_len = buffer.len(),
            "draw frame"
        );
        buffer.fill(self.color_to_pixel(THEME.surface));

        struct WindowSize {
            width: u32,
            height: u32,
        }

        let window = WindowSize {
            width: window_width,
            height: window_height,
        };

        // draw nodes to screen, starting with root
        let mut stack = vec![self.root_node];
        while let Some(taffy_id) = stack.pop() {
            let Some(taffy_node) = self.tree.get_node_context(taffy_id) else {
                unreachable!();
            };

            // early look for kids
            // pop, pops an element from the end of the vector
            if let Ok(children) = self.tree.children(taffy_id) {
                for child in children.iter().rev() {
                    stack.push(*child);
                }
            }

            let Some(node) = self.nodes_state.get_mut(taffy_node) else {
                if taffy_id != self.root_node {
                    unreachable!("Failed to fetch node from nodes_state");
                } else {
                    // skip drawing
                    continue;
                }
            };

            internal_draw(buffer, node, &window);
        }

        // internal draw function, so we can iterative do rendering
        fn internal_draw(buffer: &mut [u32], node: &mut Node, window: &WindowSize) {
            if !node.state.visible {
                return;
            }

            if node.dirty_screen || node.pixmap.is_none() {
                debug!(
                    node = %node.name,
                    x = node.rect.x(),
                    y = node.rect.y(),
                    width = node.rect.width(),
                    height = node.rect.height(),
                    "rasterize node"
                );
                node.draw();
            }

            let Some(pixmap) = &node.pixmap else {
                return;
            };

            let src_w = pixmap.width();
            let src_h = pixmap.height();
            trace!(
                node = %node.name,
                x = node.rect.x(),
                y = node.rect.y(),
                src_w,
                src_h,
                "blit pixmap"
            );
            let dst_y = node.rect.y() as u32;
            let dst_x = node.rect.x() as u32;
            let src = pixmap.data();

            for row in 0..src_h {
                let screen_y = dst_y + row;
                if screen_y >= window.height {
                    break;
                }

                for col in 0..src_w {
                    let screen_x = dst_x + col;
                    if screen_x >= window.width {
                        break;
                    }

                    let src_i = ((row * src_w + col) * 4) as usize;
                    let dst_i = (screen_y * window.width + screen_x) as usize;

                    buffer[dst_i] = blend_pixel(
                        [src[src_i], src[src_i + 1], src[src_i + 2], src[src_i + 3]],
                        buffer[dst_i],
                    );
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

#[derive(Clone, Copy)]
enum EventDispatch {
    Click,
    Press,
    Release,
    Hover,
}

fn node_area(node: &Node) -> f32 {
    node.rect.width() * node.rect.height()
}

fn set_bool(value: &mut bool, next: bool) -> bool {
    if *value == next {
        false
    } else {
        *value = next;
        true
    }
}

fn node_accepts_event(node: &Node, event: EventDispatch) -> bool {
    match event {
        EventDispatch::Click => node.events.caps.contains(crate::node::EventCaps::CLICK),
        EventDispatch::Press => node.events.caps.contains(crate::node::EventCaps::PRESS),
        EventDispatch::Release => node.events.caps.contains(crate::node::EventCaps::RELEASE),
        EventDispatch::Hover => node.events.caps.contains(crate::node::EventCaps::HOVER),
    }
}

fn node_has_handler(node: &Node, event: EventDispatch) -> bool {
    match event {
        EventDispatch::Click => {
            node.events.caps.contains(crate::node::EventCaps::CLICK)
                && node.events.on_click.is_some()
        }
        EventDispatch::Press => {
            node.events.caps.contains(crate::node::EventCaps::PRESS)
                && node.events.on_press.is_some()
        }
        EventDispatch::Release => {
            node.events.caps.contains(crate::node::EventCaps::RELEASE)
                && node.events.on_release.is_some()
        }
        EventDispatch::Hover => {
            node.events.caps.contains(crate::node::EventCaps::HOVER)
                && node.events.on_hover.is_some()
        }
    }
}

fn calculate_layout_measurement(
    known: Size<Option<f32>>,
    available_space: Size<AvailableSpace>,
    _node_id: NodeId,
    node: &mut Node,
    _node_name: &mut NodeName,
    _style: &taffy::Style,
) -> Size<f32> {
    trace!(node = %_node_name, "measure node");
    match &mut node.kind {
        NodeKind::Text(txt_content) => {
            // get max available with
            let max_width = if txt_content.is_pill {
                // ponytail: pills stay single-line; if wrapping is ever needed, re-enable width capping here.
                None
            } else {
                match available_space.width {
                    AvailableSpace::Definite(w) => Some(w),
                    _ => None,
                }
            };

            let size = FONT.measure_text(
                &txt_content.content,
                &node.style.font_size,
                max_width,
                txt_content.ellipsis,
            );
            debug!(
                node = %_node_name,
                content = %txt_content.content,
                width = size.width,
                height = size.height,
                max_width = ?max_width,
                "text measured"
            );
            size
        }
        NodeKind::Icon(icon) => {
            // println!("============");
            // println!("Known = {:?}", known);
            // println!("AvailableSpace = {:?}", available_space);

            if known.both_axis_defined() {
                let Some(new_w) = known.width else {
                    unreachable!()
                };
                let Some(new_h) = known.height else {
                    unreachable!()
                };
                icon.set_size(new_w as u32, new_h as u32);
            } else if let Some(new_w) = known.width {
                icon.set_width(new_w as u32);
            } else if let Some(new_h) = known.height {
                icon.set_height(new_h as u32);
            }

            let size = icon.get_size();
            debug!(node = %_node_name, width = size.width(), height = size.height(), "icon measured");
            Size {
                width: size.width(),
                height: size.height(),
            }
        }
        _ => Size {
            width: known.width.unwrap_or(0.0),
            height: known.height.unwrap_or(0.0),
        },
    }
}

fn blend_pixel(src: [u8; 4], dst: u32) -> u32 {
    let a = src[3] as u32;
    if a == 0 {
        return dst;
    }

    let src_r = src[0] as u32;
    let src_g = src[1] as u32;
    let src_b = src[2] as u32;
    let dst_r = (dst >> 16) & 0xff;
    let dst_g = (dst >> 8) & 0xff;
    let dst_b = dst & 0xff;
    let inv_a = 255 - a;

    let out_r = src_r + (dst_r * inv_a) / 255;
    let out_g = src_g + (dst_g * inv_a) / 255;
    let out_b = src_b + (dst_b * inv_a) / 255;

    (out_r << 16) | (out_g << 8) | out_b
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::{
        event::AppEvent,
        node::{EventCaps, NodeEvents, NodeKind, NodeName, TextContent},
    };

    use super::AppLayout;

    #[test]
    fn pointer_move_dispatches_hover() {
        let mut layout = AppLayout::new();
        let hit = Rc::new(RefCell::new(false));
        let seen = hit.clone();

        layout.create_node(
            NodeName::other("hover"),
            NodeKind::Container,
            Default::default(),
            NodeEvents {
                caps: EventCaps::HOVER,
                on_click: None,
                on_press: None,
                on_release: None,
                on_hover: Some(Rc::new(RefCell::new(Box::new(move |_, _| {
                    *seen.borrow_mut() = true;
                })))),
            },
            None,
        );

        layout.handle_event(AppEvent::PointerMove { x: 0.5, y: 0.5 });

        assert!(*hit.borrow());
    }

    #[test]
    fn tick_updates_clock_text() {
        let mut layout = AppLayout::new();

        layout.create_node(
            NodeName::Clock,
            NodeKind::Text(TextContent::new("old")),
            Default::default(),
            NodeEvents::default(),
            None,
        );

        layout.handle_event(AppEvent::Tick);

        let node = layout.nodes_state.get(&NodeName::Clock).unwrap();
        let NodeKind::Text(text) = &node.kind else {
            panic!("clock node is not text");
        };
        assert_ne!(text.content, "old");
        assert!(node.dirty_layout);
        assert!(node.dirty_screen);
    }

    #[test]
    fn pointer_events_update_node_state() {
        let mut layout = AppLayout::new();
        layout.create_node(
            NodeName::other("stateful"),
            NodeKind::Container,
            Default::default(),
            NodeEvents::default(),
            None,
        );
        layout
            .nodes_state
            .get_mut(&NodeName::other("stateful"))
            .unwrap()
            .rect = tiny_skia::Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap();

        layout.handle_event(AppEvent::PointerMove { x: 5.0, y: 5.0 });
        let node = layout
            .nodes_state
            .get(&NodeName::other("stateful"))
            .unwrap();
        assert!(node.state.hovered);
        assert!(!node.state.pressed);
        assert!(!node.state.focused);

        layout.handle_event(AppEvent::PointerDown { x: 5.0, y: 5.0 });
        let node = layout
            .nodes_state
            .get(&NodeName::other("stateful"))
            .unwrap();
        assert!(node.state.pressed);
        assert!(node.state.focused);

        layout.handle_event(AppEvent::PointerUp { x: 5.0, y: 5.0 });
        let node = layout
            .nodes_state
            .get(&NodeName::other("stateful"))
            .unwrap();
        assert!(!node.state.pressed);
        assert!(node.state.focused);
    }

    #[test]
    fn click_prefers_smallest_clickable_node() {
        let mut layout = AppLayout::new();
        let cell_clicked = Rc::new(RefCell::new(false));
        let item_clicked = Rc::new(RefCell::new(false));

        let cell_seen = cell_clicked.clone();
        layout.create_node(
            NodeName::other("cell"),
            NodeKind::Container,
            Default::default(),
            NodeEvents {
                caps: EventCaps::CLICK,
                on_click: Some(Rc::new(RefCell::new(Box::new(move |_, _| {
                    *cell_seen.borrow_mut() = true;
                })))),
                on_press: None,
                on_release: None,
                on_hover: None,
            },
            None,
        );

        layout.create_node(
            NodeName::other("text-child"),
            NodeKind::Text(TextContent::new("child")),
            Default::default(),
            NodeEvents {
                caps: EventCaps::empty(),
                on_click: None,
                on_press: None,
                on_release: None,
                on_hover: None,
            },
            None,
        );

        let item_seen = item_clicked.clone();
        layout.create_node(
            NodeName::other("item"),
            NodeKind::Container,
            Default::default(),
            NodeEvents {
                caps: EventCaps::CLICK,
                on_click: Some(Rc::new(RefCell::new(Box::new(move |_, _| {
                    *item_seen.borrow_mut() = true;
                })))),
                on_press: None,
                on_release: None,
                on_hover: None,
            },
            None,
        );

        layout
            .nodes_state
            .get_mut(&NodeName::other("cell"))
            .unwrap()
            .rect = tiny_skia::Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap();
        layout
            .nodes_state
            .get_mut(&NodeName::other("text-child"))
            .unwrap()
            .rect = tiny_skia::Rect::from_xywh(0.0, 0.0, 100.0, 100.0).unwrap();
        layout
            .nodes_state
            .get_mut(&NodeName::other("item"))
            .unwrap()
            .rect = tiny_skia::Rect::from_xywh(10.0, 10.0, 20.0, 20.0).unwrap();

        layout.handle_event(AppEvent::PointerClick { x: 15.0, y: 15.0 });

        assert!(*item_clicked.borrow());
        assert!(!*cell_clicked.borrow());
    }
}
