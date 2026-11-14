use itertools::Itertools;
use napi::Env;
use napi_derive::napi;
use sugar_path::SugarPath;

use rolldown_plugin::__inner::infer_module_def_format;
use rolldown_plugin::{PluginContext, SharedNativePluginContext};

use super::types::{
  binding_emitted_asset::BindingEmittedAsset,
  binding_emitted_chunk::BindingEmittedChunk,
  binding_emitted_prebuilt_chunk::BindingEmittedPrebuiltChunk,
  binding_hook_side_effects::BindingHookSideEffects,
  binding_import_attributes::{
    BindingImportAttributes, from_binding_import_attributes, to_binding_import_attributes,
  },
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
  binding_resolved_external::BindingResolvedExternal,
};

use crate::{
  types::{
    binding_module_info::BindingModuleInfo,
    binding_saved_module_options::{
      BindingModuleMetaEntry, BindingSavedModuleOptions, stamp_from_js, stamp_to_js,
    },
  },
  utils::napi_error,
};

#[napi]
pub struct BindingPluginContext {
  inner: SharedNativePluginContext,
}

#[napi]
impl BindingPluginContext {
  /// The module task of this hook call gets no saved result in the persistent cache. The
  /// JavaScript side calls it for what Rust cannot see, such as a `meta` value that cannot be
  /// saved.
  #[napi]
  pub fn mark_not_cacheable(&self, reason: String) {
    self.inner.mark_not_cacheable(|| reason);
  }

  /// Records a log that a JavaScript plugin sent to `onLog` during this hook call, so the
  /// persistent cache can send it again when it reuses the result. `log` is its JSON.
  #[napi]
  pub fn record_log(&self, level: String, log: String) {
    self.inner.record_js_log(rolldown_common::LogLevel::from(level), log);
  }

  /// Whether the persistent cache records this hook call, and its module task can still get a
  /// saved result.
  #[napi]
  pub fn is_cache_recording(&self) -> bool {
    self.inner.is_cache_recording()
  }

  /// The module of the task that this hook call belongs to, or `null` when the persistent cache
  /// does not record the call. In a nested `this.resolve` call, it can differ from the importer.
  #[napi]
  pub fn cache_module_id(&self) -> Option<String> {
    self.inner.cache_module_id().map(ToString::to_string)
  }

  /// Records the `meta` keys and the `moduleSideEffects` that a hook returned for a module
  /// during this hook call. The persistent cache gives them back to the JavaScript side when it
  /// reuses the result. `module_key` is the key of that module in `moduleOptionMap`.
  #[napi(
    ts_args_type = "moduleKey: string, meta: Array<BindingModuleMetaEntry>, moduleSideEffects?: boolean | 'no-treeshake'"
  )]
  pub fn record_module_options(
    &self,
    module_key: String,
    meta: Vec<BindingModuleMetaEntry>,
    module_side_effects: Option<BindingHookSideEffects>,
  ) -> napi::Result<()> {
    let module_key = arcstr::ArcStr::from(module_key);
    let side_effects = module_side_effects.map(TryInto::try_into).transpose()?;
    for BindingModuleMetaEntry { key, json } in meta {
      self.inner.record_js_module_meta(module_key.clone(), key, json);
    }
    if let Some(value) = side_effects {
      self.inner.record_js_module_side_effects(module_key, value);
    }
    Ok(())
  }

  /// The `moduleSideEffects` that this `load` or `transform` call returns to Rust is not a
  /// result of the hook. An earlier hook of the module task returned it. See
  /// `ModuleCacheRecorder::ignore_next_hook_side_effects`.
  #[napi]
  pub fn ignore_next_hook_side_effects(&self) {
    self.inner.ignore_next_hook_side_effects();
  }

  /// Like `BindingModuleInfo::saved_module_options`, for a module that the JavaScript side has
  /// no module info for. `module_key` is the key of the module in `moduleOptionMap`.
  #[napi]
  pub fn get_saved_module_options(
    &self,
    module_key: String,
    after: f64,
  ) -> Option<BindingSavedModuleOptions> {
    self
      .inner
      .saved_js_module_options()?
      .newer_than(&module_key, stamp_from_js(after))
      .map(Into::into)
  }

  #[napi(
    ts_args_type = "rawId: string, attributes: Record<string, string>, sideEffects: boolean | 'no-treeshake' | undefined, packageJsonPath?: string"
  )]
  pub async fn load(
    &self,
    raw_id: String,
    attributes: BindingImportAttributes,
    side_effects: Option<BindingHookSideEffects>,
    package_json_path: Option<String>,
  ) -> napi::Result<()> {
    let package_json = package_json_path
      .as_ref()
      .map(|p| self.inner.try_get_package_json_or_create(p.as_path()))
      .transpose()?;
    let module_def_format = infer_module_def_format(&raw_id, package_json.as_ref());
    self
      .inner
      .load(
        &raw_id,
        from_binding_import_attributes(attributes),
        side_effects.map(TryInto::try_into).transpose()?,
        module_def_format,
      )
      .await
      .map_err(|program_err| napi_error::load_error(&raw_id, program_err))
  }

  #[napi]
  pub async fn resolve(
    &self,
    specifier: String,
    importer: Option<String>,
    extra_options: Option<BindingPluginContextResolveOptions>,
  ) -> napi::Result<Option<BindingPluginContextResolvedId>> {
    let ret = self
      .inner
      .resolve(
        &specifier,
        importer.as_deref(),
        extra_options.map(TryInto::try_into).transpose().map_err(napi::Error::from_reason)?,
      )
      .await
      .map_err(|program_err| napi_error::resolve_error(&specifier, program_err))?
      .ok();

    let saved = self.inner.saved_js_module_options();
    Ok(ret.map(|info| BindingPluginContextResolvedId {
      saved_module_options_stamp: saved.and_then(|saved| {
        let key = saved.key_of_resolved(info.id.raw_id(), &info.attributes);
        saved.last_stamp_of(&key).map(stamp_to_js)
      }),
      // TODO: should use `&str` instead. (claude code) Attempt failed due to NAPI object field must be String type
      id: info.id.raw_id().to_string(),
      attributes: to_binding_import_attributes(&info.attributes).unwrap_or_default(),
      external: info.external.into(),
      module_side_effects: info.side_effects.map(Into::into),
      // TODO: should use `&str` instead. (claude code) Attempt failed due to PathBuf conversion requires to_string_lossy()
      package_json_path:
        info.package_json.map(|item| item.realpath().to_string_lossy().to_string()),
    }))
  }

  #[napi]
  pub fn emit_file<'env>(
    &self,
    env: &'env Env,
    file: BindingEmittedAsset,
    asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  ) -> napi::Result<napi::JsString<'env>> {
    let reference_id = self
      .inner
      .emit_file(file.into(), asset_filename, fn_sanitized_file_name)
      .map_err(|e| napi::Error::from_reason(e.to_string()))?;
    env.create_string(&reference_id)
  }

  #[napi]
  // Must stay sync — making this `async` would require `block_on` again
  // and reopen the emit-chunk deadlock. See `FileEmitter::emit_chunk`.
  pub fn emit_chunk<'env>(
    &self,
    env: &'env Env,
    file: BindingEmittedChunk,
  ) -> napi::Result<napi::JsString<'env>> {
    let arc_str = self.inner.emit_chunk(file.try_into()?)?;
    env.create_string(arc_str)
  }

  #[napi]
  pub fn emit_prebuilt_chunk<'env>(
    &self,
    env: &'env Env,
    file: BindingEmittedPrebuiltChunk,
  ) -> napi::Result<napi::JsString<'env>> {
    let arc_str = self.inner.emit_prebuilt_chunk(file.try_into()?);
    env.create_string(arc_str)
  }

  #[napi]
  pub fn get_file_name<'env>(
    &self,
    env: &'env Env,
    reference_id: String,
  ) -> napi::Result<napi::JsString<'env>> {
    let arc_str = self.inner.get_file_name(reference_id.as_str())?;
    env.create_string(arc_str)
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> Option<BindingModuleInfo> {
    self.inner.get_module_info(&module_id).map(|info| {
      BindingModuleInfo::with_saved_js_module_options(info, self.inner.saved_js_module_options())
    })
  }

  #[napi]
  pub fn get_module_ids<'env>(&self, env: &'env Env) -> napi::Result<Vec<napi::JsString<'env>>> {
    self.inner.get_module_ids().iter().map(|id| env.create_string(id)).try_collect()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) {
    self.inner.add_watch_file(&file);
  }
}

impl From<PluginContext> for BindingPluginContext {
  fn from(ctx: PluginContext) -> Self {
    match ctx {
      PluginContext::Napi(_) => unreachable!("Js plugins don't have PluginContext::Napi"),
      PluginContext::Native(inner) => Self { inner },
    }
  }
}
#[napi_derive::napi(object, object_from_js = false)]
pub struct BindingPluginContextResolvedId {
  /// The resolved id without its import attributes.
  pub id: String,
  #[napi(ts_type = "Record<string, string>")]
  pub attributes: BindingImportAttributes,
  pub package_json_path: Option<String>,
  #[napi(ts_type = "boolean | 'absolute' | 'relative'")]
  pub external: BindingResolvedExternal,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
  /// Set when the persistent cache holds values for the resolved module that the JavaScript
  /// side may not have yet. It is the stamp of the newest one. The JavaScript side then takes
  /// the values with `getSavedModuleOptions`. The values themselves are not copied here: a
  /// `meta` value can be large, and a plugin can resolve the same module many times.
  pub saved_module_options_stamp: Option<f64>,
}
