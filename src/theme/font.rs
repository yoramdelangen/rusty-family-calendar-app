use std::{path::PathBuf, sync::Mutex};

use cosmic_text::{
    Attrs, Buffer, Ellipsize, EllipsizeHeightLimit, Family, FontSystem, Metrics, Shaping,
    SwashCache, Weight, Wrap,
};
use once_cell::sync::Lazy;
use taffy::CoreStyle;
use tiny_skia::{Paint, Pixmap, Transform};
use tracing::debug;

use crate::node::Node;

const DEFAULT_FONT_FAMILY: &str = "Zed Mono";

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
        let mut system = FontSystem::new();
        for dir in font_dirs() {
            if dir.exists() {
                system.db_mut().load_fonts_dir(dir);
            }
        }

        Self {
            system: Mutex::new(system),
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
        ellipsis: bool,
    ) -> taffy::Size<f32> {
        let attrs = default_attrs();
        let metrics = Metrics::new(fs.font_size, fs.line_height);
        let mut system = self.system.lock().unwrap();

        let mut buffer = Buffer::new(&mut system, metrics);

        if ellipsis {
            buffer.set_wrap(Wrap::None);
            buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        }

        buffer.set_size(max_width, None);
        buffer.set_text(content, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(&mut system, false);

        let mut width: f32 = 0.0;
        let mut height = 0.0;

        for run in buffer.layout_runs() {
            let line_width = run.glyphs.iter().fold(None::<(f32, f32)>, |bounds, glyph| {
                Some(match bounds {
                    Some((min_x, max_x)) => (min_x.min(glyph.x), max_x.max(glyph.x + glyph.w)),
                    None => (glyph.x, glyph.x + glyph.w),
                })
            });

            width = width.max(line_width.map_or(run.line_w, |(min_x, max_x)| max_x - min_x));
            height += run.line_height;
        }

        debug!(content, width, height, max_width = ?max_width, "measure text");

        taffy::Size {
            width: width.ceil(),
            height: height.ceil(),
        }
    }

    pub fn draw_on_canvas(&self, canvas: &mut Pixmap, node: &Node, content: &str) {
        let attrs = default_attrs();
        let metrics = Metrics::new(
            node.style.font_size.font_size,
            node.style.font_size.line_height,
        );

        let padding = node.style.layout.padding();
        let padding_top = padding.top.into_raw().value();
        let padding_left = padding.left.into_raw().value();
        let padding_horizontal = padding_left + padding.right.into_raw().value();
        let padding_vertical = padding_top + padding.bottom.into_raw().value();
        let available_width = (node.rect.width() - padding_horizontal).max(0.0);

        let mut system = self.system.lock().unwrap();
        let mut cache = self.cache.lock().unwrap();
        let mut buffer = Buffer::new(&mut system, metrics);
        let ellipsis = matches!(
            &node.kind,
            crate::node::NodeKind::Text(txt_content) if txt_content.ellipsis
        );

        if ellipsis {
            buffer.set_wrap(Wrap::None);
            buffer.set_ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));
        }

        buffer.set_size(
            Some(available_width),
            Some(if ellipsis {
                node.style.font_size.line_height
            } else {
                node.rect.height() - padding_vertical
            }),
        );
        buffer.set_text(content, &attrs, Shaping::Advanced, node.style.text_align);
        buffer.shape_until_scroll(&mut system, false);

        let mut paint = Paint::default();
        paint.anti_alias = true;
        let text_color = node.style.text_color.to_color_u8();

        debug!(
            node = %node.name,
            width = node.rect.width(),
            height = node.rect.height(),
            padding_left,
            padding_top,
            padding_horizontal,
            padding_vertical,
            "draw text"
        );

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

fn default_attrs() -> Attrs<'static> {
    Attrs::new()
        .family(Family::Name(DEFAULT_FONT_FAMILY))
        .weight(Weight::LIGHT)
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(path) = std::env::var_os("RUSTY_CALENDAR_PI_FONT_DIR") {
        dirs.push(PathBuf::from(path));
    }

    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("rusty-calendar-pi/fonts"));
    }

    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/rusty-calendar-pi/fonts"));
    }

    dirs.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts"));
    dirs
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measure_text_honors_max_width_and_rounds_up() {
        let fonts = FontTheme::new();
        let size = FontSize::new_calc(16.0, 1.5);
        let measured = fonts.measure_text("a long enough line to wrap", &size, Some(40.0), false);

        assert!(measured.width <= 40.0);
        assert_eq!(measured.width.fract(), 0.0);
        assert_eq!(measured.height.fract(), 0.0);
    }
}
