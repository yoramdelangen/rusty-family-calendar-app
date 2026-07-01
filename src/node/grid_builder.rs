use taffy::{Display, NodeId, prelude::fr};
use tiny_skia::Color;

use crate::{
    THEME,
    components::grid_item,
    node::{
        NodeKind, NodeName,
        builder::{BobTheBuilder, Builder},
    },
};

#[derive(Debug, Clone)]
pub struct GridConfig {
    rows: Option<usize>,
    columns: usize,
}

#[derive(Debug)]
pub struct GridBuilder {
    config: GridConfig,
    builder: super::builder::Builder,
    children: Vec<super::builder::Builder>,
}

impl GridBuilder {
    // TODO: columns/rows change to direct GridConfig
    pub fn new(name: &str, columns: usize, rows: Option<usize>) -> Self {
        Self {
            config: GridConfig { rows, columns },
            builder: Builder::new(NodeKind::Grid(GridConfig { rows, columns }), None)
                .name(NodeName::Grid(name.to_owned()))
                .width_full()
                .height_full()
                .display(Display::Grid)
                .layout(|l| {
                    l.flex_grow = 1.;
                    l.flex_shrink = 1.;
                    l.grid_template_columns = vec![fr(1.0); columns];
                    if let Some(rows_count) = rows {
                        l.grid_template_rows = vec![fr(1.0); rows_count];
                    }
                })
                .border_color(THEME.raw.base09),
            children: if let Some(row_count) = rows {
                println!("Calculating children amount = {}", columns * row_count);
                let mut x = Vec::with_capacity(columns * row_count);
                x.resize_with(columns * row_count, || grid_item("cell"));
                x
            } else {
                Vec::new()
            },
        }
    }

    pub fn parent_node(mut self, parent_node_id: NodeId) -> Self {
        self.builder.parent_node = Some(parent_node_id);
        self
    }

    pub fn border_color(mut self, color: Color) -> Self {
        self.builder.style.border_color = Some(color);
        self
    }
}

impl BobTheBuilder for GridBuilder {
    fn build(mut self, layout: &mut crate::layout::AppLayout) -> taffy::NodeId {
        let conf = self.config;
        for (i, mut kid) in self.children.into_iter().enumerate() {
            // make every cell-item unique
            kid.set_name(NodeName::GridItem(format!("{}[{}]", self.builder.name, i)));

            if let Some(border_color) = self.builder.style.border_color {
                kid.set_border_color(border_color);
            }
            // Skip left border if first cell of the grid
            if i % conf.columns != 0 {
                kid.set_border_l(1.);
            } else {
                // first column on the row
            }

            self.builder.add_child(kid);
        }

        self.builder.build(layout)
    }
}
