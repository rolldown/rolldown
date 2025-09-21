use std::sync::Arc;

use arcstr::ArcStr;
use napi::{
  Env,
  bindgen_prelude::{AsyncBlock, AsyncBlockBuilder},
};
use napi_derive::napi;
use rolldown::ModuleType;
use rolldown_common::WatcherChangeKind;
use rolldown_plugin::{
  CustomField, HookLoadArgs, HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput,
  HookTransformArgs, PluginIdx, Pluginable, SharedTransformPluginContext, TransformPluginContext,
};
use rolldown_plugin_vite_resolve::ResolveIdOptionsScan;
use rolldown_utils::unique_arc::UniqueArc;

use crate::options::plugin::types::{
  binding_hook_side_effects::BindingHookSideEffects,
  binding_hook_transform_output::BindingHookTransformOutput,
  binding_plugin_transform_extra_args::BindingTransformHookExtraArgs,
};

use super::{
  binding_builtin_plugin::BindingBuiltinPlugin,
  types::binding_resolved_external::BindingResolvedExternal,
  types::binding_vite_plugin_custom::BindingVitePluginCustom,
};

#[napi]
pub struct BindingCallableBuiltinPlugin {
  inner: Arc<dyn Pluginable>,
  context: SharedTransformPluginContext,
}

#[napi]
impl BindingCallableBuiltinPlugin {
  #[napi(constructor)]
  pub fn new(plugin: BindingBuiltinPlugin) -> napi::Result<Self> {
    let inner: Arc<dyn Pluginable> = plugin.try_into()?;

    Ok(Self {
      inner,
      context: Arc::new(TransformPluginContext::new(
        rolldown_plugin::PluginContext::new_napi_context(),
        UniqueArc::new(vec![]).weak_ref(),
        ArcStr::default(),
        ArcStr::default(),
        rolldown_common::ModuleIdx::new(0),
        PluginIdx::new(0),
        None,
      )),
    })
  }

  #[napi]
  pub fn resolve_id(
    &self,
    env: Env,
    id: String,
    importer: Option<String>,
    options: Option<BindingHookJsResolveIdOptions>,
  ) -> napi::Result<AsyncBlock<Option<BindingHookJsResolveIdOutput>>> {
    let plugin = Arc::clone(&self.inner);
    let context = Arc::clone(&self.context);
    crate::start_async_runtime();
    AsyncBlockBuilder::with(async move {
      plugin
        .call_resolve_id(
          &context.inner,
          &HookResolveIdArgs {
            specifier: &id,
            importer: importer.as_deref(),
            is_entry: options.as_ref().is_some_and(|options| options.is_entry.unwrap_or_default()),
            kind: rolldown_common::ImportKind::Import,
            custom: options.map(Into::into).unwrap_or_default(),
          },
        )
        .await
        .map_err(AnyHowMaybeNapiError::into_napi_error)
        .map(|result| result.map(Into::into))
    })
    .with_dispose(|_| {
      crate::shutdown_async_runtime();
      Ok(())
    })
    .build(&env)
  }

  #[napi]
  pub fn load(
    &self,
    env: Env,
    id: String,
  ) -> napi::Result<AsyncBlock<Option<BindingHookJsLoadOutput>>> {
    let plugin = Arc::clone(&self.inner);
    let context = Arc::clone(&self.context);
    crate::start_async_runtime();
    AsyncBlockBuilder::with(async move {
      plugin
        .call_load(&context.inner, &HookLoadArgs { id: &id })
        .await
        .map_err(AnyHowMaybeNapiError::into_napi_error)
        .map(|result| result.map(Into::into))
    })
    .with_dispose(|_| {
      crate::shutdown_async_runtime();
      Ok(())
    })
    .build(&env)
  }

  #[napi]
  pub fn transform(
    &self,
    env: Env,
    code: String,
    id: String,
    options: BindingTransformHookExtraArgs,
  ) -> napi::Result<AsyncBlock<Option<BindingHookTransformOutput>>> {
    let module_type = ModuleType::from_known_str(&options.module_type)?;
    let plugin = Arc::clone(&self.inner);
    let context = Arc::clone(&self.context);
    crate::start_async_runtime();
    AsyncBlockBuilder::with(async move {
      plugin
        .call_transform(
          context,
          &HookTransformArgs { id: &id, code: &code, module_type: &module_type },
        )
        .await
        .map_err(AnyHowMaybeNapiError::into_napi_error)
        .map(|result| result.map(Into::into))
    })
    .with_dispose(|_| {
      crate::shutdown_async_runtime();
      Ok(())
    })
    .build(&env)
  }

  #[napi]
  pub fn watch_change(
    &self,
    env: Env,
    path: String,
    event: BindingJsWatchChangeEvent,
  ) -> napi::Result<AsyncBlock<()>> {
    let kind = event.bindingify_watcher_change_kind()?;
    let plugin = Arc::clone(&self.inner);
    let context = Arc::clone(&self.context);
    crate::start_async_runtime();
    AsyncBlockBuilder::with(async move {
      plugin
        .call_watch_change(&context.inner, &path, kind)
        .await
        .map_err(AnyHowMaybeNapiError::into_napi_error)
    })
    .with_dispose(|_| {
      crate::shutdown_async_runtime();
      Ok(())
    })
    .build(&env)
  }
}

#[derive(Debug)]
#[napi(object, object_to_js = false)]
pub struct BindingHookJsResolveIdOptions {
  pub is_entry: Option<bool>,
  pub scan: Option<bool>,
  pub custom: Option<BindingVitePluginCustom>,
}

impl From<BindingHookJsResolveIdOptions> for Arc<CustomField> {
  fn from(value: BindingHookJsResolveIdOptions) -> Self {
    let map = CustomField::default();
    map.insert(ResolveIdOptionsScan, value.scan.unwrap_or(false));
    if let Some(is_sub_imports_pattern) =
      value.custom.and_then(|v| v.vite_import_glob.and_then(|v| v.is_sub_imports_pattern))
    {
      map.insert(
        rolldown_plugin_utils::constants::ViteImportGlob,
        rolldown_plugin_utils::constants::ViteImportGlobValue(is_sub_imports_pattern),
      );
    }
    Arc::new(map)
  }
}

#[napi(object)]
pub struct BindingHookJsResolveIdOutput {
  pub id: String,
  #[napi(ts_type = "boolean | 'absolute' | 'relative'")]
  pub external: Option<BindingResolvedExternal>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
}

impl From<HookResolveIdOutput> for BindingHookJsResolveIdOutput {
  fn from(value: HookResolveIdOutput) -> Self {
    Self {
      id: value.id.to_string(),
      external: value.external.map(Into::into),
      module_side_effects: value.side_effects.map(Into::into),
    }
  }
}

#[napi(object)]
pub struct BindingHookJsLoadOutput {
  pub code: String,
  pub map: Option<String>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub module_side_effects: Option<BindingHookSideEffects>,
}

impl From<HookLoadOutput> for BindingHookJsLoadOutput {
  fn from(value: HookLoadOutput) -> Self {
    Self {
      code: value.code.to_string(),
      map: value.map.map(|map| map.to_json_string()),
      module_side_effects: value.side_effects.map(Into::into),
    }
  }
}

#[napi(object)]
pub struct BindingJsWatchChangeEvent {
  pub event: String,
}

impl BindingJsWatchChangeEvent {
  fn bindingify_watcher_change_kind(&self) -> napi::Result<WatcherChangeKind> {
    match self.event.as_str() {
      "create" => Ok(WatcherChangeKind::Create),
      "delete" => Ok(WatcherChangeKind::Delete),
      "update" => Ok(WatcherChangeKind::Update),
      _ => Err(napi::Error::new(napi::Status::InvalidArg, "Invalid watcher change kind")),
    }
  }
}

trait AnyHowMaybeNapiError {
  fn into_napi_error(self) -> napi::Error;
}

impl AnyHowMaybeNapiError for anyhow::Error {
  fn into_napi_error(self) -> napi::Error {
    match self.downcast::<napi::Error>() {
      Ok(napi_error) => napi_error,
      Err(original_error) => original_error.into(),
    }
  }
}
