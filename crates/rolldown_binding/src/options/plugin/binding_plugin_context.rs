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
  binding_plugin_context_resolve_options::BindingPluginContextResolveOptions,
  binding_resolved_external::BindingResolvedExternal,
};

use crate::{
  types::{
    binding_module_info::BindingModuleInfo,
    external_memory_status::{ExternalMemoryStatus, release_arc},
  },
  utils::{napi_error, spawn_boxed_future},
};

#[napi]
pub struct BindingPluginContext {
  inner: Option<SharedNativePluginContext>,
}

#[napi]
impl BindingPluginContext {
  fn try_get_inner(&self) -> napi::Result<&SharedNativePluginContext> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this plugin context's native data was eagerly released after its hook invocation settled. Use the context only while the hook runs.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    release_arc(&mut self.inner)
  }

  #[napi(
    ts_args_type = "rawId: string, attributes: Record<string, string>, sideEffects: boolean | 'no-treeshake' | undefined, resolveDependencies: boolean, packageJsonPath?: string",
    ts_return_type = "Promise<void>"
  )]
  pub fn load<'env>(
    &self,
    env: &'env Env,
    raw_id: String,
    attributes: BindingImportAttributes,
    side_effects: Option<BindingHookSideEffects>,
    resolve_dependencies: bool,
    package_json_path: Option<String>,
  ) -> napi::Result<PromiseRaw<'env, ()>> {
    let inner = Arc::clone(self.try_get_inner()?);
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
    let inner = Arc::clone(self.try_get_inner()?);
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
      // Report the value this id would be bundled with, not only a hook return.
      // `ResolvedId::side_effects` stays the raw hook value so loading the module
      // still runs the normal side-effect decision.
      let module_side_effects = inner
        .reported_module_side_effects(&info)
        .await
        .map_err(|err| napi::Error::from_reason(err.to_string()))?;
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
      .try_get_inner()?
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
    let arc_str = self.try_get_inner()?.emit_chunk(file.try_into()?)?;
    env.create_string(arc_str)
  }

  #[napi]
  pub fn emit_prebuilt_chunk<'env>(
    &self,
    env: &'env Env,
    file: BindingEmittedPrebuiltChunk,
  ) -> napi::Result<napi::JsString<'env>> {
    let arc_str = self.try_get_inner()?.emit_prebuilt_chunk(file.try_into()?);
    env.create_string(arc_str)
  }

  #[napi]
  pub fn get_file_name<'env>(
    &self,
    env: &'env Env,
    reference_id: String,
  ) -> napi::Result<napi::JsString<'env>> {
    let arc_str = self.try_get_inner()?.get_file_name(reference_id.as_str())?;
    env.create_string(arc_str)
  }

  #[napi]
  pub fn get_module_info(&self, module_id: String) -> napi::Result<Option<BindingModuleInfo>> {
    Ok(self.try_get_inner()?.get_module_info(&module_id).map(BindingModuleInfo::new))
  }

  #[napi]
  pub fn get_module_ids<'env>(&self, env: &'env Env) -> napi::Result<Vec<napi::JsString<'env>>> {
    self.try_get_inner()?.get_module_ids().iter().map(|id| env.create_string(id)).try_collect()
  }

  #[napi]
  pub fn add_watch_file(&self, file: String) -> napi::Result<()> {
    self.try_get_inner()?.add_watch_file(&file);
    Ok(())
  }
}

impl From<PluginContext> for BindingPluginContext {
  fn from(ctx: PluginContext) -> Self {
    match ctx {
      PluginContext::Napi(_) => unreachable!("Js plugins don't have PluginContext::Napi"),
      PluginContext::Native(inner) => Self { inner: Some(inner) },
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
}
