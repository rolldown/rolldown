use napi::bindgen_prelude::Either;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingExperimentalOptions {
  pub strict_execution_order: Option<bool>,
  pub disable_live_bindings: Option<bool>,
  pub vite_mode: Option<bool>,
  pub resolve_new_url_to_asset: Option<bool>,
  pub hmr: Option<BindingExperimentalHmrOptions>,
  pub attach_debug_info: Option<BindingAttachDebugInfo>,
  pub chunk_modules_order: Option<BindingChunkModuleOrderBy>,
  pub chunk_import_map: Option<Either<bool, BindingChunkImportMap>>,
  pub on_demand_wrapping: Option<bool>,
  pub incremental_build: Option<bool>,
  #[napi(ts_type = "boolean | 'boundary'")]
  pub transform_hires_sourcemap: Option<Either<bool, String>>,
}

impl TryFrom<BindingExperimentalOptions> for rolldown_common::ExperimentalOptions {
  type Error = napi::Error;

  fn try_from(value: BindingExperimentalOptions) -> Result<Self, Self::Error> {
    Ok(Self {
      strict_execution_order: value.strict_execution_order,
      disable_live_bindings: value.disable_live_bindings,
      vite_mode: value.vite_mode,
      resolve_new_url_to_asset: value.resolve_new_url_to_asset,
      incremental_build: value.incremental_build,
      hmr: value.hmr.map(Into::into),
      attach_debug_info: value.attach_debug_info.map(Into::into),
      chunk_modules_order: value.chunk_modules_order.map(Into::into),
      chunk_import_map: value.chunk_import_map.and_then(|v| match v {
        Either::A(v) => v.then_some(rolldown_common::ChunkImportMap::default()),
        Either::B(v) => Some(v.into()),
      }),
      on_demand_wrapping: value.on_demand_wrapping,
      transform_hires_sourcemap: if let Some(v) = value.transform_hires_sourcemap {
        match v {
          Either::A(v) => Some(rolldown_common::SourcemapHires::Boolean(v)),
          Either::B(v) => {
            if v == "boundary" {
              Some(rolldown_common::SourcemapHires::Boundary)
            } else {
              return Err(napi::Error::new(
                napi::Status::InvalidArg,
                format!("Invalid transform hires sourcemap: {v}"),
              ));
            }
          }
        }
      } else {
        None
      },
    })
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

#[napi_derive::napi(object)]
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
