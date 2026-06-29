use crate::node::{NodeKind, builder::Builder};

pub fn div() -> Builder {
    Builder::new(NodeKind::Container, None).width_full()
}
