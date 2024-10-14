use oxc::transformer::ReplaceGlobalDefinesConfig;
use rolldown_common::{
  side_effects::HookSideEffects, ModuleIdx, ModuleType, ResolvedId, StrOrBytes,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::SourceMap;

use crate::SharedOptions;

pub struct CreateModuleContext<'a> {
  pub module_index: ModuleIdx,
  pub plugin_driver: &'a SharedPluginDriver,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<BuildDiagnostic>,
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}

pub struct CreateModuleViewArgs {
  pub source: StrOrBytes,
  pub sourcemap_chain: Vec<SourceMap>,
  pub hook_side_effects: Option<HookSideEffects>,
}
