use napi::bindgen_prelude::Either;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingExperimentalOptions {
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub dev_mode: Option<BindingExperimentalDevModeOptions>,
  pub attach_debug_info: Option<BindingAttachDebugInfo>,
  pub chunk_modules_order: Option<BindingChunkModuleOrderBy>,
  pub chunk_import_map: Option<Either<bool, BindingChunkImportMap>>,
  pub on_demand_wrapping: Option<bool>,
  pub incremental_build: Option<bool>,
  pub native_magic_string: Option<bool>,
  pub chunk_optimization: Option<bool>,
  pub lazy_barrel: Option<bool>,
}

impl TryFrom<BindingExperimentalOptions> for rolldown_common::ExperimentalOptions {
  type Error = napi::Error;

  fn try_from(value: BindingExperimentalOptions) -> Result<Self, Self::Error> {
    Ok(Self {
      vite_mode: value.vite_mode,
      resolve_new_url_to_asset: value.resolve_new_url_to_asset,
      incremental_build: value.incremental_build,
      dev_mode: value.dev_mode.map(Into::into),
      attach_debug_info: value.attach_debug_info.map(Into::into),
      chunk_modules_order: value.chunk_modules_order.map(Into::into),
      chunk_import_map: value.chunk_import_map.and_then(|v| match v {
        Either::A(v) => v.then_some(rolldown_common::ChunkImportMap::default()),
        Either::B(v) => Some(v.into()),
      }),
      on_demand_wrapping: value.on_demand_wrapping,
      native_magic_string: value.native_magic_string,
      chunk_optimization: value.chunk_optimization,
      lazy_barrel: value.lazy_barrel,
    })
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingExperimentalDevModeOptions {
  pub host: Option<String>,
  pub port: Option<u16>,
  pub implement: Option<String>,
  pub lazy: Option<bool>,
}

impl From<BindingExperimentalDevModeOptions> for rolldown_common::DevModeOptions {
  fn from(value: BindingExperimentalDevModeOptions) -> Self {
    Self { host: value.host, port: value.port, implement: value.implement, lazy: value.lazy }
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

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingChunkImportMap {
  pub base_url: Option<String>,
  pub file_name: Option<String>,
}

impl From<BindingChunkImportMap> for rolldown_common::ChunkImportMap {
  fn from(value: BindingChunkImportMap) -> Self {
    Self { base_url: value.base_url, file_name: value.file_name }
  }
}
