use std::sync::Arc;

use napi::{bindgen_prelude::FromNapiValue, Either};
use napi_derive::napi;
use rolldown_common::{side_effects, WatcherChangeKind};
use rolldown_plugin::{
  CustomField, HookLoadArgs, HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput,
};
use rolldown_plugin_vite_resolve::{
  CallablePluginAsyncTrait, ResolveIdOptionsScan, ViteResolvePlugin,
};

use super::{
  binding_builtin_plugin::{BindingBuiltinPlugin, BindingViteResolvePluginConfig},
  types::binding_builtin_plugin_name::BindingBuiltinPluginName,
};

impl TryFrom<BindingBuiltinPlugin> for Arc<dyn CallablePluginAsyncTrait> {
  type Error = napi::Error;

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::ViteResolve => {
        let config = if let Some(options) = plugin.options {
          BindingViteResolvePluginConfig::from_unknown(options)?
        } else {
          return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "Missing options for ViteResolvePlugin",
          ));
        };

        Arc::new(ViteResolvePlugin::new(config.into()))
      }
      _ => return Err(napi::Error::new(napi::Status::InvalidArg, "Non-callable builtin plugin.")),
    })
  }
}

#[napi]
pub struct BindingCallableBuiltinPlugin {
  inner: Arc<dyn CallablePluginAsyncTrait>,
}

#[napi]
impl BindingCallableBuiltinPlugin {
  #[napi(constructor)]
  pub fn new(plugin: BindingBuiltinPlugin) -> napi::Result<Self> {
    let inner: Arc<dyn CallablePluginAsyncTrait> = plugin.try_into()?;

    Ok(Self { inner })
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
        .resolve_id(&HookResolveIdArgs {
          specifier: &id,
          importer: importer.as_deref(),
          is_entry: false,
          kind: rolldown_common::ImportKind::Import,
          custom: options.map(Into::into).unwrap_or_default(),
        })
        .await?
        .map(Into::into),
    )
  }

  #[napi]
  pub async fn load(&self, id: String) -> napi::Result<Option<BindingHookJsLoadOutput>> {
    Ok(self.inner.load(&HookLoadArgs { id: &id }).await?.map(Into::into))
  }

  #[napi]
  pub async fn watch_change(
    &self,
    path: String,
    event: BindingJsWatchChangeEvent,
  ) -> napi::Result<()> {
    self.inner.watch_change(&path, bindingify_watcher_change_kind(event.event)?).await?;
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
  pub external: Option<bool>,
  #[napi(ts_type = "boolean | 'no-treeshake'")]
  pub side_effects: BindingJsSideEffects,
}

impl From<HookResolveIdOutput> for BindingHookJsResolveIdOutput {
  fn from(value: HookResolveIdOutput) -> Self {
    Self {
      id: value.id,
      external: value.external,
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
