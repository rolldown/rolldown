use std::sync::Arc;

use derivative::Derivative;
use rolldown_plugin::{typedmap::TypedDashMap, PluginContextResolveOptions};
use serde::Deserialize;

use crate::options::plugin::JsPluginContextResolveCustomArgId;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Derivative, Default)]
#[serde(rename_all = "camelCase")]
#[derivative(Debug)]
pub struct BindingPluginContextResolveOptions {
  #[napi(ts_type = "'import' | 'dynamic-import' | 'require-call'")]
  pub import_kind: Option<String>,
  pub skip_self: Option<bool>,
  pub custom: Option<u32>,
}

impl TryFrom<BindingPluginContextResolveOptions> for PluginContextResolveOptions {
  type Error = String;

  fn try_from(value: BindingPluginContextResolveOptions) -> Result<Self, Self::Error> {
    let custom = TypedDashMap::new();
    if let Some(js_custom_id) = value.custom {
      custom.insert(JsPluginContextResolveCustomArgId, js_custom_id);
    }
    Ok(Self {
      import_kind: value.import_kind.as_deref().unwrap_or("import").try_into()?,
      skip_self: value.skip_self.unwrap_or(true),
      custom: Arc::new(custom),
    })
  }
}
