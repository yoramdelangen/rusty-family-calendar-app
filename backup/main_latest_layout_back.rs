use std::collections::HashMap;

use taffy::{
    Dimension, FlexDirection, NodeId, Point, Rect, Size, Style, TaffyTree,
    prelude::{auto, fr, length, percent},
};
use tiny_skia::{Color, Paint, Pixmap, Transform};

type TaffyTreeHouse = TaffyTree<Node>;

const ROWS_COUNT: usize = 4;
const COLUMNS_COUNT: usize = 7;

#[derive(Debug)]
enum NodeContent {
    None,
    Text(TextContext),
}

#[derive(Debug)]
struct TextContext {
    content: String,
    font_size: f32,
    color: tiny_skia::Color,
}

#[derive(Debug, Clone)]
struct NodeStyle {
    border_color: Option<Color>,
    border_size: Rect<f32>,
    border_radius: f32,
    background_color: Option<Color>,
    width: Dimension,
    height: Dimension,
    display: taffy::Display,
    flex_grow: f32,
    flex_shrink: f32,
    flex_direction: FlexDirection,
    grid_template: Option<GridTemplate>,
}

#[derive(Debug, Clone)]
struct GridTemplate {
    columns: usize,
    rows: usize,
}

impl Default for NodeStyle {
    fn default() -> Self {
        Self {
            border_color: None,
            border_radius: 0.0,
            border_size: Rect::zero(),
            background_color: None,
            width: auto(),
            height: auto(),
            display: taffy::Display::default(),
            flex_grow: 1.0,
            flex_shrink: 1.0,
            flex_direction: FlexDirection::default(),
            grid_template: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct NodeState {
    hovered: bool,
    pressed: bool,
    focused: bool,
    visible: bool,
}

impl NodeState {
    fn visible() -> Self {
        Self {
            visible: true,
            ..Self::default()
        }
    }
}

#[derive(Debug)]
struct Node {
    id: NodeName,
    node_id: Option<NodeId>,
    style: NodeStyle,
    content: NodeContent,
    state: NodeState,
    offset: Point<f32>,
    rect: tiny_skia::Rect,
    pixmap: Option<Pixmap>,
    dirty_layout: bool,
    dirty_screen: bool,
}

impl Node {
    fn background_color(mut self, color: Color) -> Self {
        self.style.background_color = Some(color);
        self
    }

    fn width_full(mut self) -> Self {
        self.style.width = percent(1.0);
        self
    }

    fn height_full(mut self) -> Self {
        self.style.height = percent(1.0);
        self
    }

    fn height(mut self, h: Dimension) -> Self {
        self.style.height = h;
        self
    }

    fn height_auto(mut self) -> Self {
        self.style.height = auto();
        self
    }

    fn display(mut self, display: taffy::Display) -> Self {
        self.style.display = display;
        self
    }

    fn grid_template(mut self, columns: usize, rows: usize) -> Self {
        self.style.grid_template = Some(GridTemplate { columns, rows });
        self
    }

    fn border_b(mut self, b: f32) -> Self {
        self.style.border_size.bottom = b;
        self
    }

    fn border_t(mut self, t: f32) -> Self {
        self.style.border_size.top = t;
        self
    }

    fn border_t_1(self) -> Self {
        self.border_t(1.0)
    }

    fn border_color(mut self, color: Color) -> Self {
        self.style.border_color = Some(color);
        self
    }

    fn flex_direction(mut self, direction: FlexDirection) -> Self {
        self.style.flex_direction = direction;
        self
    }

    fn flex_grow(mut self, fg: f32) -> Self {
        self.style.flex_grow = fg;
        self
    }

    fn flex_shrink(mut self, fg: f32) -> Self {
        self.style.flex_shrink = fg;
        self
    }

    fn add(self, layout: &mut AppLayout) -> NodeId {
        layout.add(self)
    }

    fn add_children(self, layout: &mut AppLayout, children: &[NodeId]) -> NodeId {
        let node_id = if let Some(node_id) = self.node_id {
            node_id
        } else {
            layout.add(self)
        };

        layout.add_children(node_id, children);
        node_id
    }

    fn draw(&mut self) {
        if !self.dirty_screen {
            return;
        }

        if self.rect.width() <= 0.0 || self.rect.height() <= 0.0 {
            return;
        }

        let mut canvas = Pixmap::new(self.rect.width() as u32, self.rect.height() as u32)
            .expect("failed creating node pixmap");

        if let Some(bg_color) = self.style.background_color {
            canvas.fill(bg_color);
        }

        let border = self.style.border_size;

        fn draw_border(
            canvas: &mut Pixmap,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            color: &Paint,
        ) {
            canvas.fill_rect(
                tiny_skia::Rect::from_xywh(x, y, width, height).expect("invalid border rect"),
                color,
                Transform::identity(),
                None,
            );
        }

        if let Some(border_color) = self.style.border_color {
            let mut border_paint = Paint::default();
            border_paint.set_color(border_color);

            if border.top > 0.0 {
                draw_border(&mut canvas, 0.0, 0.0, self.rect.width(), border.top, &border_paint);
            }
            if border.bottom > 0.0 {
                draw_border(
                    &mut canvas,
                    0.0,
                    self.rect.height() - border.bottom,
                    self.rect.width(),
                    border.bottom,
                    &border_paint,
                );
            }
            if border.left > 0.0 {
                draw_border(&mut canvas, 0.0, 0.0, border.left, self.rect.height(), &border_paint);
            }
            if border.right > 0.0 {
                draw_border(
                    &mut canvas,
                    self.rect.width() - border.right,
                    0.0,
                    border.right,
                    self.rect.height(),
                    &border_paint,
                );
            }
        }

        self.pixmap = Some(canvas);
        self.dirty_screen = false;
    }
}

fn div(id: NodeName) -> Node {
    Node {
        id,
        node_id: None,
        style: NodeStyle::default(),
        content: NodeContent::None,
        state: NodeState::visible(),
        offset: Point::zero(),
        rect: tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("invalid default rect"),
        pixmap: None,
        dirty_layout: true,
        dirty_screen: true,
    }
}

fn grid(id: String) -> Node {
    Node {
        id: NodeName::grid(&id),
        node_id: None,
        style: NodeStyle::default(),
        content: NodeContent::None,
        state: NodeState::visible(),
        offset: Point::zero(),
        rect: tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("invalid default rect"),
        pixmap: None,
        dirty_layout: true,
        dirty_screen: true,
    }
    .width_full()
    .height_auto()
    .display(taffy::Display::Grid)
    .grid_template(COLUMNS_COUNT, ROWS_COUNT)
}

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
enum NodeName {
    Header,
    Footer,
    Content,
    Grid(String),
    Other(String),
}

impl NodeName {
    fn grid(name: &str) -> Self {
        Self::Grid(name.to_owned())
    }

    fn other(name: &str) -> Self {
        Self::Other(name.to_owned())
    }
}

struct AppLayout {
    root_node: NodeId,
    tree: TaffyTreeHouse,
    nodes: HashMap<NodeId, NodeName>,
    state: HashMap<NodeName, Node>,
}

impl AppLayout {
    fn new() -> Self {
        let mut tree: TaffyTreeHouse = TaffyTree::new();
        let nodes = HashMap::new();
        let state = HashMap::new();

        let root_node = tree
            .new_leaf(Style {
                flex_direction: FlexDirection::Column,
                ..Default::default()
            })
            .expect("failed creating root node");
        tree.mark_dirty(root_node).expect("failed marking root dirty");

        Self {
            root_node,
            tree,
            nodes,
            state,
        }
    }

    fn add(&mut self, node: Node) -> NodeId {
        let node_id = self.tree.new_leaf(Style::DEFAULT).expect("failed creating leaf");
        self.nodes.insert(node_id, node.id.clone());
        self.state.insert(node.id.clone(), node);
        self.tree
            .add_child(self.root_node, node_id)
            .expect("failed adding child to root");
        node_id
    }

    fn add_children(&mut self, parent: NodeId, children: &[NodeId]) {
        self.tree
            .set_children(parent, children)
            .expect("failed setting children");
    }

    pub(crate) fn render_layout(&mut self, size: Size<Dimension>) {
        if self.tree.dirty(self.root_node).expect("failed dirty lookup") {
            let root = self.tree.style(self.root_node).expect("missing root style").clone();
            self.tree
                .set_style(
                    self.root_node,
                    Style {
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
            .compute_layout(self.root_node, Size::max_content())
            .expect("failed computing layout");
        self.compute_layout_nodes(self.root_node);
    }

    fn prepare_layout_leafs(&mut self, id: NodeId) {
        let children = self.tree.children(id).expect("failed child lookup").to_vec();

        for node_id in children {
            let node_name = self.nodes.get(&node_id).expect("missing node name").clone();
            let node_state = self
                .state
                .get_mut(&node_name)
                .expect("missing node state while computing layout");

            if node_state.dirty_layout || self.tree.dirty(node_id).expect("dirty lookup failed") {
                let prev_style = self.tree.style(node_id).expect("missing style").clone();

                self.tree
                    .set_style(
                        node_id,
                        Style {
                            size: Size {
                                width: node_state.style.width,
                                height: node_state.style.height,
                            },
                            border: {
                                let bs = node_state.style.border_size;
                                Rect {
                                    left: length(bs.left),
                                    right: length(bs.right),
                                    bottom: length(bs.bottom),
                                    top: length(bs.top),
                                }
                            },
                            display: node_state.style.display,
                            flex_direction: node_state.style.flex_direction,
                            flex_grow: node_state.style.flex_grow,
                            flex_shrink: node_state.style.flex_shrink,
                            grid_template_rows: if let Some(template) = &node_state.style.grid_template {
                                vec![fr(1.0); template.rows]
                            } else {
                                vec![]
                            },
                            grid_template_columns: if let Some(template) = &node_state.style.grid_template {
                                vec![fr(1.0); template.columns]
                            } else {
                                vec![]
                            },
                            ..prev_style
                        },
                    )
                    .expect("failed updating leaf style");
            }

            self.prepare_layout_leafs(node_id);
        }
    }

    fn compute_layout_nodes(&mut self, id: NodeId) {
        let children = self.tree.children(id).expect("failed child lookup").to_vec();

        for node_id in children {
            let node_name = self.nodes.get(&node_id).expect("missing node name").clone();
            let node_state = self
                .state
                .get_mut(&node_name)
                .expect("missing node state while computing layout");

            if node_state.dirty_layout || self.tree.dirty(node_id).expect("dirty lookup failed") {
                let node = self.tree.layout(node_id).expect("missing layout node");
                let offset_x = node.location.x + node.padding.left;
                let offset_y = node.location.y + node.padding.top;

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

            self.compute_layout_nodes(node_id);
        }
    }

    pub(crate) fn draw(&mut self, buffer: &mut [u32], window_width: u32, window_height: u32) {
        buffer.fill(0);

        for node in self.state.values_mut() {
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
                    buffer[dst_i] = (r << 16) | (g << 8) | b;
                }
            }
        }
    }
}
