use std::sync::Arc;

use itertools::Itertools;
use napi::{Env, bindgen_prelude::PromiseRaw};
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
  binding_module_meta_entry::BindingModuleMetaEntry,
  binding_plugin_context_load_options::BindingPluginContextLoadOptions,
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
  binding_resolved_external::BindingResolvedExternal,
};

use crate::{
  types::{
    binding_log_level::BindingLogLevel,
    binding_module_info::{
      BindingModuleInfo, BindingModuleSideEffectsOptionFn, call_module_side_effects_option,
      to_binding_module_side_effects,
    },
  },
  utils::{napi_error, spawn_boxed_future},
};

#[napi]
pub struct BindingPluginContext {
  inner: SharedNativePluginContext,
}

#[napi]
impl BindingPluginContext {
  /// The module task of this hook call gets no saved result in the persistent cache. The
  /// JavaScript side calls it for what Rust cannot see, such as a hook that returns `meta`.
  #[napi]
  pub fn mark_not_cacheable(&self, reason: String) {
    self.inner.mark_not_cacheable(|| reason);
  }

  /// Records a log that a JavaScript plugin sent to `onLog` during this hook call, so the
  /// persistent cache can send it again when it reuses the result. `log` is its JSON.
  #[napi]
  pub fn record_log(&self, level: BindingLogLevel, log: String) {
    self.inner.record_js_log(level.into(), log);
  }

  /// @internal Whether the persistent cache records this hook call, and its module task can
  /// still get a saved result.
  #[napi]
  pub fn is_cache_recording(&self) -> bool {
    self.inner.is_cache_recording()
  }

  /// @internal The module of the task that this hook call belongs to, when the persistent cache
  /// records the call.
  #[napi]
  pub fn cache_module_id(&self) -> Option<String> {
    self.inner.cache_module_id().map(ToString::to_string)
  }

  /// @internal Records the `meta` that a `load` or `transform` hook returned, so the persistent
  /// cache can give it back when it reuses the result.
  #[napi]
  pub fn record_module_meta(&self, entries: Vec<BindingModuleMetaEntry>) {
    self.inner.record_js_module_meta(entries.into_iter().map(Into::into).collect());
  }

  #[napi(
    ts_args_type = "rawId: string, attributes: Record<string, string>, sideEffects: boolean | 'no-treeshake' | undefined, resolveDependencies: boolean, options?: BindingPluginContextLoadOptions",
    ts_return_type = "Promise<void>"
  )]
  pub fn load<'env>(
    &self,
    env: &'env Env,
    raw_id: String,
    attributes: BindingImportAttributes,
    side_effects: Option<BindingHookSideEffects>,
    resolve_dependencies: bool,
    options: Option<BindingPluginContextLoadOptions>,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let BindingPluginContextLoadOptions { package_json_path, module_options_receipt } =
      options.unwrap_or_default();
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let package_json = package_json_path
        .as_ref()
        .map(|p| inner.try_get_package_json_or_create(p.as_path()))
        .transpose()?;
      let module_def_format = infer_module_def_format(&raw_id, package_json.as_ref());
      inner
        .load(
          &raw_id,
          from_binding_import_attributes(attributes),
          side_effects.map(TryInto::try_into).transpose()?,
          module_def_format,
          resolve_dependencies,
          module_options_receipt,
        )
        .await
        .map_err(|program_err| napi_error::load_error(&raw_id, program_err))
    })
  }

  #[napi(ts_return_type = "Promise<BindingPluginContextResolvedId | null>")]
  pub fn resolve<'env>(
    &self,
    env: &'env Env,
    specifier: String,
    importer: Option<String>,
    extra_options: Option<BindingPluginContextResolveOptions>,
  ) -> napi::Result<PromiseRaw<'env, Option<BindingPluginContextResolvedId>>> {
    let inner = Arc::clone(&self.inner);
    spawn_boxed_future(env, async move {
      let ret = inner
        .resolve(
          &specifier,
          importer.as_deref(),
          extra_options.map(TryInto::try_into).transpose().map_err(napi::Error::from_reason)?,
        )
        .await
        .map_err(|program_err| napi_error::resolve_error(&specifier, program_err))?
        .ok();

      let Some(info) = ret else {
        return Ok(None);
      };
      // The hook value of this resolution, as in Rollup. Without one, the value that the module
      // would get from `treeshake.moduleSideEffects`, `package.json` or the default.
      // `ResolvedId::side_effects` stays the raw hook value, so loading the module still runs the
      // normal side-effect decision.
      let module_side_effects = match info.side_effects {
        Some(value) => value,
        None => inner
          .reported_module_side_effects(&info)
          .await
          .map_err(|err| napi::Error::from_reason(err.to_string()))?,
      };
      Ok(Some(BindingPluginContextResolvedId {
        // TODO: should use `&str` instead. (claude code) Attempt failed due to NAPI object field must be String type
        id: info.id.raw_id().to_string(),
        attributes: to_binding_import_attributes(&info.attributes).unwrap_or_default(),
        external: info.external.into(),
        module_side_effects: Some(module_side_effects.into()),
        // TODO: should use `&str` instead. (claude code) Attempt failed due to PathBuf conversion requires to_string_lossy()
        package_json_path: info
          .package_json
          .map(|item| item.realpath().to_string_lossy().to_string()),
        module_options_receipt: info.module_options_receipt,
      }))
    })
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
    self.inner.get_module_info(&module_id).map(BindingModuleInfo::new)
  }

  /// `getModuleInfo(moduleId)?.moduleSideEffects`, without copying the rest of the info.
  /// `callOption` is the `treeshake.moduleSideEffects` function, if the option is one. It runs
  /// only when the value needs it and no earlier call for the module left a result.
  #[napi(
    ts_args_type = "moduleId: string, callOption?: (id: string, external: boolean) => boolean | undefined | null",
    ts_return_type = "boolean | 'no-treeshake' | null"
  )]
  pub fn get_module_side_effects(
    &self,
    module_id: String,
    call_option: Option<BindingModuleSideEffectsOptionFn<'_>>,
  ) -> napi::Result<Option<napi::Either<bool, &'static str>>> {
    let side_effects =
      self.inner.get_module_side_effects_or_call(&module_id, |raw_id, external| {
        call_module_side_effects_option(call_option.as_ref(), raw_id, external)
      })?;
    Ok(side_effects.map(to_binding_module_side_effects))
  }

  /// A write to `getModuleInfo(moduleId).moduleSideEffects`. `null` and `undefined` mean
  /// "not decided". After such a write, the module shows the value without hooks, so
  /// `callOption` runs here if that value needs it. Tree shaking then reuses the result.
  /// A write after `buildEnd` is ignored. Returns `true` for the first one in a build, so the
  /// caller can warn.
  #[napi(
    ts_args_type = "moduleId: string, sideEffects: boolean | 'no-treeshake' | null | undefined, callOption?: (id: string, external: boolean) => boolean | undefined | null"
  )]
  pub fn set_module_side_effects(
    &self,
    module_id: String,
    side_effects: Option<BindingHookSideEffects>,
    call_option: Option<BindingModuleSideEffectsOptionFn<'_>>,
  ) -> napi::Result<bool> {
    let side_effects = side_effects.map(TryInto::try_into).transpose()?;
    let should_warn = self.inner.set_module_side_effects_without_warning(&module_id, side_effects);
    if side_effects.is_none() && call_option.is_some() {
      self.inner.get_module_side_effects_or_call(&module_id, |raw_id, external| {
        call_module_side_effects_option(call_option.as_ref(), raw_id, external)
      })?;
    }
    Ok(should_warn)
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
  /// @internal See `ResolvedId::module_options_receipt`.
  pub module_options_receipt: Option<u32>,
}
