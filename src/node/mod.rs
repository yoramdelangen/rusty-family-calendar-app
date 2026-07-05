pub mod builder;
pub mod grid_builder;

use taffy::{LengthPercentage, Rect};
use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Pixmap, Point, Stroke, Transform};

use crate::{
    THEME,
    icons::IconInfo,
    node::grid_builder::GridConfig,
    theme::font::{FONT, FontSize},
};

use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
pub fn next_node_id() -> u64 {
    NEXT.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Debug)]
pub struct TextContent {
    pub content: String,
    pub is_pill: bool,
}
impl TextContent {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_pill: false,
        }
    }
}

#[derive(Clone, Debug)]
pub enum NodeKind {
    Container,
    Text(TextContent),
    Icon(IconInfo),
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

        if self.style.border_radius.ne(&Rect::zero()) {
            let rounded_rect_path = self.rounded_rect(
                0.,
                0.,
                self.rect.width(),
                self.rect.height(),
                self.style.border_radius,
            );

            if let Some(bg_color) = self.style.background_color {
                let bg_color = bg_color.to_color_u8();
                let mut bg_paint = Paint::default();
                bg_paint.set_color_rgba8(
                    bg_color.red(),
                    bg_color.green(),
                    bg_color.blue(),
                    bg_color.alpha(),
                );

                canvas.fill_path(
                    &rounded_rect_path,
                    &bg_paint,
                    FillRule::Winding,
                    Transform::default(),
                    None,
                );

                if let Some(border_color) = self.style.border_color {
                    let mut border_paint = Paint::default();
                    border_paint.set_color(border_color);

                    let stroke = Stroke {
                        width: 1.0,
                        ..Stroke::default()
                    };

                    canvas.stroke_path(
                        &rounded_rect_path,
                        &border_paint,
                        &stroke,
                        Transform::identity(),
                        None,
                    );
                }
            }
        } else {
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
        }

        // println!("DRAWING ========================== {:?}", self.kind);
        match &self.kind {
            NodeKind::Container => {}
            NodeKind::Text(txt_content) => {
                FONT.draw_on_canvas(&mut canvas, &self, &txt_content.content)
            }
            NodeKind::Grid(_) => {}
            NodeKind::GridItem => {}
            NodeKind::Icon(i) => {
                println!("DRAWING ==========================+");
                i.draw_on_canvas(&mut canvas.as_mut(), &self)
            }
        };

        self.pixmap = Some(canvas);
        self.dirty_screen = false;
    }

    fn rounded_rect(&self, x: f32, y: f32, w: f32, h: f32, r: Rect<LengthPercentage>) -> Path {
        // let r = r.min(w / 2.0).min(h / 2.0);
        let tl = r.top.into_raw().value();
        let tr = r.right.into_raw().value();
        let br = r.bottom.into_raw().value();
        let bl = r.left.into_raw().value();

        let mut pb = PathBuilder::new();

        // Start on the top edge.
        pb.move_to(x + tl, y);

        // Top-right corner.
        pb.line_to(x + w - tr, y);
        pb.quad_to(x + w, y, x + w, y + tr);

        // Bottom-right corner.
        pb.line_to(x + w, y + h - br);
        pb.quad_to(x + w, y + h, x + w - br, y + h);

        // Bottom-left corner.
        pb.line_to(x + bl, y + h);
        pb.quad_to(x, y + h, x, y + h - bl);

        // Top-left corner.
        pb.line_to(x, y + tl);
        pb.quad_to(x, y, x + tl, y);

        pb.close();
        pb.finish().unwrap()
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
    pub font_size: FontSize,
    pub border_color: Option<Color>,
    pub border_radius: Rect<LengthPercentage>,
    // pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            layout: taffy::Style::default(),
            background_color: None, // Color::TRANSPARENT,
            border_color: None,     // Color::TRANSPARENT,
            text_color: THEME.text,
            font_size: FONT.base.clone(),
            border_radius: Rect::zero(),
            // border_radius: 0.0,
            // opacity: 1.0,
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
    Root,
    Header,
    Footer,
    Content,
    Icon(String),
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

impl NodeName {
    pub fn other(name: impl Into<String>) -> Self {
        NodeName::Other(name.into())
    }

    pub fn icon(name: impl Into<String>) -> Self {
        NodeName::Icon(format!(
            "{}_{}",
            (name.into()).replace(".svg", ""),
            next_node_id()
        ))
    }
}

impl std::fmt::Display for NodeName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeName::Root => f.write_str("ROOT"),
            NodeName::Header => f.write_str("HEADER"),
            NodeName::Footer => f.write_str("FOOTER"),
            NodeName::Content => f.write_str("CONTENT"),
            NodeName::Icon(id) => write!(f, "ICON[{id}]"),
            NodeName::Grid(id) => write!(f, "GRID[{id}]"),
            NodeName::Other(id) => write!(f, "OTHER[{id}]"),
            NodeName::NoName(id) => write!(f, "NAMELESS[{id}]"),
            NodeName::GridItem(id) => write!(f, "GRID_ITEM[{id}]"),
        }
    }
}
