#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub hmr: Option<BindingExperimentalHmrOptions>,
  pub attach_debug_info: Option<BindingAttachDebugInfo>,
  pub chunk_modules_order: Option<BindingChunkModuleOrderBy>,
  pub chunk_import_map: Option<bool>,
  pub on_demand_wrapping: Option<bool>,
  pub incremental_build: Option<bool>,
}

impl From<BindingExperimentalOptions> for rolldown_common::ExperimentalOptions {
  fn from(value: BindingExperimentalOptions) -> Self {
    Self {
      strict_execution_order: value.strict_execution_order,
      disable_live_bindings: value.disable_live_bindings,
      vite_mode: value.vite_mode,
      resolve_new_url_to_asset: value.resolve_new_url_to_asset,
      incremental_build: value.incremental_build,
      hmr: value.hmr.map(Into::into),
      attach_debug_info: value.attach_debug_info.map(Into::into),
      chunk_modules_order: value.chunk_modules_order.map(Into::into),
      chunk_import_map: value.chunk_import_map,
      on_demand_wrapping: value.on_demand_wrapping,
    }
  }
}

#[napi_derive::napi(object)]
#[derive(Debug, Default)]
pub struct BindingExperimentalHmrOptions {
  pub host: Option<String>,
  pub port: Option<u16>,
  pub implement: Option<String>,
}

impl From<BindingExperimentalHmrOptions> for rolldown_common::HmrOptions {
  fn from(value: BindingExperimentalHmrOptions) -> Self {
    Self { host: value.host, port: value.port, implement: value.implement }
  }
}

#[napi_derive::napi]
#[derive(Debug)]
pub enum BindingAttachDebugInfo {
  None,
  Simple,
  Full,
}

impl From<BindingAttachDebugInfo> for rolldown_common::AttachDebugInfo {
  fn from(value: BindingAttachDebugInfo) -> Self {
    match value {
      BindingAttachDebugInfo::None => rolldown_common::AttachDebugInfo::None,
      BindingAttachDebugInfo::Simple => rolldown_common::AttachDebugInfo::Simple,
      BindingAttachDebugInfo::Full => rolldown_common::AttachDebugInfo::Full,
    }
  }
}

#[derive(Debug)]
#[napi_derive::napi]
pub enum BindingChunkModuleOrderBy {
  ModuleId,
  ExecOrder,
}

impl From<BindingChunkModuleOrderBy> for rolldown_common::ChunkModulesOrderBy {
  fn from(value: BindingChunkModuleOrderBy) -> Self {
    match value {
      BindingChunkModuleOrderBy::ModuleId => rolldown_common::ChunkModulesOrderBy::ModuleId,
      BindingChunkModuleOrderBy::ExecOrder => rolldown_common::ChunkModulesOrderBy::ExecOrder,
    }
  }
}
