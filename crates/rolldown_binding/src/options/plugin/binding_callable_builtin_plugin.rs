use std::sync::Arc;

use napi::{bindgen_prelude::FromNapiValue, Either};
use napi_derive::napi;
use rolldown_common::side_effects;
use rolldown_plugin::{HookResolveIdArgs, HookResolveIdOutput};
use rolldown_plugin_vite_resolve::{CallablePluginAsyncTrait, ViteResolvePlugin};

use super::binding_builtin_plugin::{
  BindingBuiltinPlugin, BindingBuiltinPluginName, BindingViteResolvePluginConfig,
};

#[napi]
pub fn is_callable_compatible_builtin_plugin(plugin: BindingBuiltinPlugin) -> bool {
  #[allow(clippy::match_like_matches_macro)]
  match plugin.__name {
    BindingBuiltinPluginName::ViteResolvePlugin => true,
    _ => false,
  }
}

impl TryFrom<BindingBuiltinPlugin> for Arc<dyn CallablePluginAsyncTrait> {
  type Error = napi::Error;

  fn try_from(plugin: BindingBuiltinPlugin) -> Result<Self, Self::Error> {
    Ok(match plugin.__name {
      BindingBuiltinPluginName::ViteResolvePlugin => {
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
  pub name: String,
  inner: Arc<dyn CallablePluginAsyncTrait>,
}

#[napi]
impl BindingCallableBuiltinPlugin {
  #[napi(constructor)]
  pub fn new(plugin: BindingBuiltinPlugin) -> napi::Result<Self> {
    let inner: Arc<dyn CallablePluginAsyncTrait> = plugin.try_into()?;

    Ok(Self { name: inner.name().to_string(), inner })
  }

  #[napi]
  pub async fn resolve_id(
    &self,
    id: String,
    importer: Option<String>,
  ) -> napi::Result<Option<BindingHookResolveIdReturn>> {
    Ok(
      self
        .inner
        .resolve_id(&HookResolveIdArgs {
          specifier: &id,
          importer: importer.as_deref(),
          is_entry: false,
          kind: rolldown_common::ImportKind::Import,
          custom: Arc::default(),
        })
        .await?
        .map(Into::into),
    )
  }
}

#[napi(object)]
pub struct BindingHookResolveIdReturn {
  pub id: String,
  pub external: Option<bool>,
  pub side_effects: Option<Either<bool, String>>,
}

impl From<HookResolveIdOutput> for BindingHookResolveIdReturn {
  fn from(value: HookResolveIdOutput) -> Self {
    Self {
      id: value.id,
      external: value.external,
      side_effects: value.side_effects.map(|side_effects| match side_effects {
        side_effects::HookSideEffects::False => Either::A(false),
        side_effects::HookSideEffects::True => Either::A(true),
        side_effects::HookSideEffects::NoTreeshake => Either::B("no-treeshake".to_string()),
      }),
    }
  }
}
