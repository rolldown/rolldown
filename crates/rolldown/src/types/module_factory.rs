use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::{
  FlatOptions, ModuleIdx, ModuleType, ResolvedId, StrOrBytes, side_effects::HookSideEffects,
};
use rolldown_error::BuildDiagnostic;
use rolldown_plugin::SharedPluginDriver;
use rolldown_sourcemap::SourceMap;

use crate::SharedOptions;

pub struct CreateModuleContext<'a> {
  pub stable_id: &'a str,
  pub module_index: ModuleIdx,
  pub plugin_driver: &'a SharedPluginDriver,
  pub resolved_id: &'a ResolvedId,
  pub options: &'a SharedOptions,
  pub module_type: ModuleType,
  pub warnings: &'a mut Vec<BuildDiagnostic>,
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
  pub is_user_defined_entry: bool,
  pub flat_options: FlatOptions,
}

pub struct CreateModuleViewArgs {
  pub source: StrOrBytes,
  pub sourcemap_chain: Vec<SourceMap>,
  pub hook_side_effects: Option<HookSideEffects>,
}
