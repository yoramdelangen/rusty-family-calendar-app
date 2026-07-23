use cosmic_text::Align;
use taffy::{
    AlignItems, Display, FlexDirection, JustifyContent,
    prelude::{length, percent},
};
use tiny_skia::{Color, Point};

use crate::{
    icons::IconInfo,
    node::{
        EventCaps, NodeKind, NodeName, TextContent, builder::Builder, grid_builder::GridBuilder,
        next_node_id, shape_builder::ShapeBuilder,
    },
    theme::THEME,
};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}

pub fn shape(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color)
}

pub fn circle(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).circle()
}

pub fn oval(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).oval()
}

pub fn rect(color: Color) -> ShapeBuilder {
    ShapeBuilder::new(color).rect()
}

pub fn rounded_rect(color: Color, radius: f32) -> ShapeBuilder {
    ShapeBuilder::new(color).rounded_rect(radius)
}

pub fn polygon(color: Color, points: Vec<Point>) -> ShapeBuilder {
    ShapeBuilder::new(color).polygon(points)
}

pub fn text(val: impl Into<String>) -> Builder {
    Builder::new(NodeKind::Text(TextContent::new(val)), None).events(EventCaps::empty())
}

pub fn icon(icon: &str) -> Builder {
    Builder::new(NodeKind::Icon(IconInfo::new(icon)), None).name(NodeName::icon(icon))
}

pub fn grid(name: &str, columns: usize, rows: Option<usize>) -> GridBuilder {
    GridBuilder::new(name, columns, rows)
}

pub fn grid_item(name: &str) -> Builder {
    Builder::new(NodeKind::GridItem, None)
        .name(NodeName::GridItem(format!("{}-{}", name, next_node_id())))
        .border_b_1()
        .px(4.)
        .py(4.)
        .display(Display::Flex)
        .flex_dir_column()
        .layout(|l| {
            l.max_size.width = percent(1.);
            l.min_size.width = length(0.);
            l.align_items = Some(AlignItems::FlexStart);
        })
}

pub fn pill(content: impl Into<String>) -> Builder {
    text(content)
        .events(EventCaps::all())
        .width_auto()
        .py(2.)
        .px(4.)
        .rounded_xl()
        .name(NodeName::pill(None::<String>))
        .kind_meta(|kind| {
            if let NodeKind::Text(txt_content) = kind {
                txt_content.is_pill = true;
            }
        })
        .background(THEME.surface_raised)
}

pub enum ButtonContent {
    Icon(&'static str),
    Text(String),
    IconText {
        icon: &'static str,
        text: String,
        icon_position: IconPosition,
    },
}

pub enum IconPosition {
    Before,
    After,
}

pub fn button(content: ButtonContent) -> Builder {
    let mut kids = Vec::with_capacity(2);
    let mut icon_only = false;

    match content {
        ButtonContent::Icon(icon_name) => {
            icon_only = true;
            kids.push(icon(icon_name).width(16.).height(16.));
        }
        ButtonContent::Text(label) => {
            kids.push(text(label).text_align(Align::Center));
        }
        ButtonContent::IconText {
            icon: icon_name,
            text: label,
            icon_position,
        } => {
            let icon = icon(icon_name).width(16.).height(16.);
            let text = text(label).text_align(Align::Center).px(4.);

            match icon_position {
                IconPosition::Before => {
                    kids.push(icon);
                    kids.push(text);
                }
                IconPosition::After => {
                    kids.push(text);
                    kids.push(icon);
                }
            }
        }
    }

    let button = Builder::new(NodeKind::Container, None)
        .events(EventCaps::CLICK)
        .width_auto()
        .height_auto()
        .rounded(999.)
        .background(THEME.surface_raised)
        .border_color(THEME.border)
        .border_t(1.)
        .border_b(1.)
        .border_l(1.)
        .border_r(1.)
        .layout(|l| {
            l.flex_direction = FlexDirection::Row;
            l.align_items = Some(AlignItems::Center);
            l.justify_content = Some(JustifyContent::Center);
        })
        .children(kids);

    if icon_only {
        button.width(36.).height(36.)
    } else {
        button.px(12.).py(8.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_starts_without_events() {
        assert_eq!(text("hello").events.caps, EventCaps::empty());
    }
}
