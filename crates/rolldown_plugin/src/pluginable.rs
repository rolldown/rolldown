use std::sync::Arc;

use crate::Plugin;

pub type SharedPluginable = Arc<dyn Plugin>;
