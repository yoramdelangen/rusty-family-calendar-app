pub mod builder;
pub mod grid_builder;

use tiny_skia::{Color, Paint, Pixmap, Point, Transform};

use crate::node::grid_builder::GridConfig;

use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn next_node_id() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    Container,
    // Text(String),
    // Image(ImageNodeId),
    // Canvas(CanvasNodeId),
    Grid(GridConfig),
    GridItem,
}

#[derive(Debug)]
pub struct Node {
    pub taffy_id: taffy::NodeId,
    pub kind: NodeKind,
    pub name: NodeName,
    pub style: Style,
    pub children: Vec<taffy::NodeId>,
    pub state: State,
    pub offset: Point,
    pub rect: tiny_skia::Rect,
    pub pixmap: Option<Pixmap>,
    pub dirty_layout: bool,
    pub dirty_screen: bool,
}

impl Node {
    pub fn new(node_id: taffy::NodeId, name: NodeName, kind: NodeKind, style: Style) -> Self {
        Node {
            taffy_id: node_id,
            kind,
            name,
            style,
            children: Vec::new(),
            state: State::default(),
            offset: Point::zero(),
            rect: tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("invalid default rect"),
            pixmap: None,
            dirty_layout: true,
            dirty_screen: true,
        }
    }

    pub fn draw(&mut self) {
        if !self.dirty_screen && !self.dirty_layout {
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

        let border = self.style.layout.border;

        if let Some(border_color) = self.style.border_color {
            let mut border_paint = Paint::default();
            border_paint.set_color(border_color);

            let b_top = border.top.into_raw().value();
            let b_bottom = border.bottom.into_raw().value();
            let b_left = border.left.into_raw().value();
            let b_right = border.right.into_raw().value();

            draw_border(
                &mut canvas,
                0.0,
                0.0,
                self.rect.width(),
                b_top,
                b_top,
                &border_paint,
            );

            draw_border(
                &mut canvas,
                0.0,
                self.rect.height() - b_bottom,
                self.rect.width(),
                b_bottom,
                b_bottom,
                &border_paint,
            );

            draw_border(
                &mut canvas,
                0.0,
                0.0,
                b_left,
                self.rect.height(),
                b_left,
                &border_paint,
            );

            draw_border(
                &mut canvas,
                self.rect.width() - b_right,
                0.0,
                b_right,
                self.rect.height(),
                b_right,
                &border_paint,
            );
        }

        self.pixmap = Some(canvas);
        self.dirty_screen = false;
    }
}

fn draw_border(
    canvas: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    size: f32,
    color: &Paint,
) {
    if size <= 0. {
        return;
    }

    canvas.fill_rect(
        tiny_skia::Rect::from_xywh(x, y, width, height).expect("invalid border rect"),
        color,
        Transform::identity(),
        None,
    );
}

#[derive(Debug, Clone)]
pub struct Style {
    pub layout: taffy::Style,

    pub background_color: Option<Color>,
    pub text_color: Color,
    pub border_color: Option<Color>,
    pub border_radius: f32,
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            layout: taffy::Style::default(),
            background_color: None, // Color::TRANSPARENT,
            text_color: Color::BLACK,
            border_color: None, // Color::TRANSPARENT,
            border_radius: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Debug)]
pub struct State {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    pub visible: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hovered: false,
            pressed: false,
            focused: false,
            visible: true,
        }
    }
}

#[derive(Eq, Hash, PartialEq, Clone, Debug)]
pub enum NodeName {
    Header,
    Footer,
    Content,
    Grid(String),
    GridItem(String),
    Other(String),
    NoName(u64),
}

impl Default for NodeName {
    fn default() -> Self {
        NodeName::NoName(next_node_id())
    }
}

impl std::fmt::Display for NodeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeName::Header => f.write_str("HEADER"),
            NodeName::Footer => f.write_str("FOOTER"),
            NodeName::Content => f.write_str("CONTENT"),
            NodeName::Grid(id) => write!(f, "GRID[{id}]"),
            NodeName::Other(id) => write!(f, "OTHER[{id}]"),
            NodeName::NoName(id) => write!(f, "NAMELESS[{id}]"), //f.write_str("NAMELESS"),
            NodeName::GridItem(id) => write!(f, "GRID_ITEM[{id}]"),
        }
    }
}
