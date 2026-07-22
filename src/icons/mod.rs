use std::path::{Path, PathBuf};

use tiny_skia::{IntSize, PixmapMut, Transform};
use usvg::{Options, Size, Tree};

use crate::{node::Node, theme::color_to_hex};

pub fn read_svg(path: &str, opts: Options) -> Tree {
    let svg_file = std::fs::read(path)
        .map_err(|e| format!("{} - {}", e, path))
        .expect("Icon not found");

    Tree::from_data(&svg_file, &opts)
        .map_err(|e| format!("{} - {}", e, path))
        .unwrap()
}

#[derive(Clone, Debug)]
pub struct IconInfo {
    path: String,
    size: IntSize,
}

impl IconInfo {
    pub fn new(path: &str) -> Self {
        let icon_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
        let path = if Path::new(path).exists() {
            PathBuf::from(path)
        } else if icon_dir.join(path).exists() {
            icon_dir.join(path)
        } else if icon_dir.join(format!("{path}.svg")).exists() {
            icon_dir.join(format!("{path}.svg"))
        } else {
            unreachable!("icon does not exist: {path}")
        };
        let path = path.to_string_lossy().to_string();
        let tree = read_svg(&path, usvg::Options::default());

        Self {
            path,
            size: tree.size().to_int_size(),
        }
    }

    pub fn get_size(&self) -> Size {
        self.size.to_size()
    }

    pub fn set_width(&mut self, w: u32) {
        self.size = self.size.scale_to_width(w).unwrap();
    }

    pub fn set_height(&mut self, h: u32) {
        self.size = self.size.scale_to_height(h).unwrap();
    }

    pub fn set_size(&mut self, w: u32, h: u32) {
        self.size = tiny_skia::IntSize::from_wh(w, h)
            .map(|s| self.size.scale_to(s))
            .unwrap();
    }

    pub fn draw_on_canvas(&self, canvas: &mut PixmapMut, node: &Node) {
        let color_hex = color_to_hex(node.style.text_color);

        // re-read the svg and inject css
        let mut opts = usvg::Options::default();
        opts.style_sheet = Some(format!("* {{ fill: {}; }}", color_hex));
        let tree = read_svg(&self.path, opts);

        // calculate resize if needed
        let tree_size = tree.size();
        let target_size = self.size.to_size();
        let scale = (target_size.width() / tree_size.width())
            .min(target_size.height() / tree_size.height());

        let transform = Transform::from_scale(scale, scale);

        resvg::render(&tree, transform, canvas);
    }
}

// fn change_colors(group: &Group) {
//     for g in group.children() {
//         if let usvg::Node::Path(path) = g {
//             path.fill
//             let Some(node_fill) = path.fill() else {
//                 continue
//             };
//             println!("NODE {:#?}", path);
//             // println!("NODE fill {:#?}", );
//             let paint = node_fill.paint();
//
//             //
//         }
//
//         if let usvg::Node::Group(grp) = g {
//             change_colors(grp);
//         }
//
//         g.subroots(|group| change_colors(group));
//     }
// }
