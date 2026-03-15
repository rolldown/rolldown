use oxc_index::IndexVec;
use rolldown_common::PluginIdx;

use crate::{PluginContext, pluginable::SharedPluginable};

pub type IndexPluginable = IndexVec<PluginIdx, SharedPluginable>;
pub type IndexPluginContext = IndexVec<PluginIdx, PluginContext>;
