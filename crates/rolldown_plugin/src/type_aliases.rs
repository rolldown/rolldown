use oxc_index::IndexVec;

use crate::{__inner::SharedPluginable, types::plugin_idx::PluginIdx, SharedPluginContext};

pub type IndexPluginable = IndexVec<PluginIdx, SharedPluginable>;
pub type IndexPluginContext = IndexVec<PluginIdx, SharedPluginContext>;
