use std::sync::Arc;

use napi::Either;
use napi_derive::napi;
use rolldown_common::{WatcherChangeKind, side_effects};
use rolldown_plugin::{
  CustomField, HookLoadArgs, HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput, Pluginable,
};
use rolldown_plugin_vite_resolve::ResolveIdOptionsScan;

use super::{
  binding_builtin_plugin::BindingBuiltinPlugin,
  types::binding_resolved_external::BindingResolvedExternal,
};

#[napi]
pub struct BindingCallableBuiltinPlugin {
  inner: Arc<dyn Pluginable>,
  context: rolldown_plugin::PluginContext,
}

#[napi]
impl BindingCallableBuiltinPlugin {
  #[napi(constructor)]
  pub fn new(plugin: BindingBuiltinPlugin) -> napi::Result<Self> {
    let inner: Arc<dyn Pluginable> = plugin.try_into()?;

    Ok(Self { inner, context: rolldown_plugin::PluginContext::new_napi_context() })
  }

  #[napi]
  pub async fn resolve_id(
    &self,
    id: String,
    importer: Option<String>,
    options: Option<BindingHookJsResolveIdOptions>,
  ) -> napi::Result<Option<BindingHookJsResolveIdOutput>> {
    Ok(
      self
        .inner
        .call_resolve_id(
          &self.context,
          &HookResolveIdArgs {
            specifier: &id,
            importer: importer.as_deref(),
            is_entry: false,
            kind: rolldown_common::ImportKind::Import,
            custom: options.map(Into::into).unwrap_or_default(),
          },
        )
        .await?
        .map(Into::into),
    )
  }

  #[napi]
  pub async fn load(&self, id: String) -> napi::Result<Option<BindingHookJsLoadOutput>> {
    Ok(self.inner.call_load(&self.context, &HookLoadArgs { id: &id }).await?.map(Into::into))
  }

  #[napi]
  pub async fn watch_change(
    &self,
    path: String,
    event: BindingJsWatchChangeEvent,
  ) -> napi::Result<()> {
    self
      .inner
      .call_watch_change(&self.context, &path, bindingify_watcher_change_kind(event.event)?)
      .await?;
    Ok(())
  }
}

#[derive(Debug)]
#[napi(object)]
pub struct BindingHookJsResolveIdOptions {
  pub scan: Option<bool>,
}

impl From<BindingHookJsResolveIdOptions> for Arc<CustomField> {
  fn from(value: BindingHookJsResolveIdOptions) -> Self {
    let map = CustomField::default();
    map.insert(ResolveIdOptionsScan {}, value.scan.unwrap_or(false));
    Arc::new(map)
  }
}

#[napi(object)]
pub struct BindingHookJsResolveIdOutput {
  pub id: String,
  #[napi(ts_type = "boolean | 'absolute' | 'relative'")]
  pub external: Option<BindingResolvedExternal>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub side_effects: BindingJsSideEffects,
}

impl From<HookResolveIdOutput> for BindingHookJsResolveIdOutput {
  fn from(value: HookResolveIdOutput) -> Self {
    Self {
      id: value.id.to_string(),
      external: value.external.map(Into::into),
      side_effects: get_side_effects_binding(value.side_effects),
    }
  }
}

#[napi(object)]
pub struct BindingHookJsLoadOutput {
  pub code: String,
  pub map: Option<String>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub side_effects: BindingJsSideEffects,
}

impl From<HookLoadOutput> for BindingHookJsLoadOutput {
  fn from(value: HookLoadOutput) -> Self {
    Self {
      code: value.code,
      map: value.map.map(|map| map.to_json_string()),
      side_effects: get_side_effects_binding(value.side_effects),
    }
  }
}

type BindingJsSideEffects = Option<Either<bool, String>>;

fn get_side_effects_binding(value: Option<side_effects::HookSideEffects>) -> BindingJsSideEffects {
  value.map(|side_effects| match side_effects {
    side_effects::HookSideEffects::False => Either::A(false),
    side_effects::HookSideEffects::True => Either::A(true),
    side_effects::HookSideEffects::NoTreeshake => Either::B("no-treeshake".to_string()),
  })
}

#[napi(object)]
pub struct BindingJsWatchChangeEvent {
  pub event: String,
}

fn bindingify_watcher_change_kind(value: String) -> napi::Result<WatcherChangeKind> {
  match value.as_str() {
    "create" => Ok(WatcherChangeKind::Create),
    "delete" => Ok(WatcherChangeKind::Delete),
    "update" => Ok(WatcherChangeKind::Update),
    _ => Err(napi::Error::new(napi::Status::InvalidArg, "Invalid watcher change kind")),
  }
}
