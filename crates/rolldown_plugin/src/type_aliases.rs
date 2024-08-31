use oxc_index::IndexVec;

use crate::types::hook_filter::HookFilterOptions;
use crate::{__inner::SharedPluginable, types::plugin_idx::PluginIdx, PluginContext};

pub type IndexPluginable = IndexVec<PluginIdx, SharedPluginable>;
pub type IndexPluginContext = IndexVec<PluginIdx, PluginContext>;
pub type IndexPluginFilter = IndexVec<PluginIdx, HookFilterOptions>;
