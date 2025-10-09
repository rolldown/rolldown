use oxc_index::IndexVec;
use rolldown_common::PluginIdx;

use crate::{__inner::SharedPluginable, PluginContext};

pub type IndexPluginable = IndexVec<PluginIdx, SharedPluginable>;
pub type IndexPluginContext = IndexVec<PluginIdx, PluginContext>;
