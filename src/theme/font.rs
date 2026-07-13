use std::sync::Mutex;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use once_cell::sync::Lazy;
use taffy::CoreStyle;
use tiny_skia::{Paint, Pixmap, Transform};

use crate::node::Node;

pub(crate) static FONT: Lazy<FontTheme> = Lazy::new(FontTheme::new);

pub(crate) struct FontTheme {
    system: Mutex<FontSystem>,
    cache: Mutex<SwashCache>,

    // Sizes based on tailwind
    pub xs: FontSize,
    pub sm: FontSize,
    pub base: FontSize,
    pub lg: FontSize,
    pub xl: FontSize,
    pub xxl: FontSize,
    pub xxxl: FontSize,
    pub xxxxl: FontSize,
}

impl FontTheme {
    pub fn new() -> Self {
        Self {
            system: Mutex::new(FontSystem::new()),
            cache: Mutex::new(SwashCache::new()),

            // Sizes inspired by Tailwindcss
            xs: FontSize::new_calc(12., 1. / 0.75),
            sm: FontSize::new_calc(14., 1.25 / 0.875),
            base: FontSize::new_calc(16., 1.5 / 1.),
            lg: FontSize::new_calc(18., 1.75 / 1.125),
            xl: FontSize::new_calc(20., 1.75 / 1.25),
            xxl: FontSize::new_calc(24., 2. / 1.5),
            xxxl: FontSize::new_calc(30., 2.25 / 1.875),
            xxxxl: FontSize::new_calc(36., 2.5 / 2.25),
        }
    }

    pub fn measure_text(
        &self,
        content: &str,
        fs: &FontSize,
        max_width: Option<f32>,
    ) -> taffy::Size<f32> {
        let attrs = Attrs::new();
        let metrics = Metrics::new(fs.font_size, fs.line_height);
        let mut system = self.system.lock().unwrap();

        let mut buffer = Buffer::new(&mut system, metrics);

        buffer.set_size(max_width, None);
        buffer.set_text(content, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut system, false);

        let width = buffer
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0, f32::max);

        let height = buffer.layout_runs().map(|run| run.line_height).sum::<f32>();

        taffy::Size { width, height }
    }

    pub fn draw_on_canvas(&self, canvas: &mut Pixmap, node: &Node, content: &str) {
        let attrs = Attrs::new();
        let metrics = Metrics::new(
            node.style.font_size.font_size,
            node.style.font_size.line_height,
        );

        let padding = node.style.layout.padding();
        let padding_top = padding.top.into_raw().value();
        let padding_left = padding.left.into_raw().value();
        let padding_horizontal = padding_left + padding.right.into_raw().value();
        let padding_vertical = padding_top + padding.bottom.into_raw().value();

        let mut system = self.system.lock().unwrap();
        let mut cache = self.cache.lock().unwrap();
        let mut buffer = Buffer::new(&mut system, metrics);
        buffer.set_size(
            Some(node.rect.width() - padding_horizontal),
            Some(node.rect.height() - padding_vertical),
        );
        buffer.set_text(
            &content,
            &attrs,
            Shaping::Advanced,
            node.style.text_align,
        );
        buffer.shape_until_scroll(&mut system, false);

        let mut paint = Paint::default();
        paint.anti_alias = true;
        let text_color = node.style.text_color.to_color_u8();

        buffer.draw(
            &mut system,
            &mut cache,
            cosmic_text::Color::rgba(
                text_color.red(),
                text_color.green(),
                text_color.blue(),
                text_color.alpha(),
            ),
            |x, y, w, h, color| {
                paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());

                canvas.fill_rect(
                    tiny_skia::Rect::from_xywh(
                        (x as f32) + padding_left,
                        (y as f32) + padding_top,
                        w as f32,
                        h as f32,
                    )
                    .unwrap(),
                    &paint,
                    Transform::identity(),
                    None,
                );
            },
        );
    }
}

#[derive(Clone, Debug)]
pub struct FontSize {
    pub font_size: f32,
    pub line_height: f32,
}

impl FontSize {
    pub fn new_calc(font_size: f32, line_height_factor: f32) -> Self {
        Self {
            font_size,
            line_height: font_size * line_height_factor,
        }
    }
    // pub fn new(size: f32, line_height: f32) -> Self {
    //     Self { size, line_height }
    // }
}
