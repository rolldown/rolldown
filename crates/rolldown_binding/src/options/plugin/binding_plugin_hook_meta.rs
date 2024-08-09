use napi_derive::napi;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[napi]
pub enum BindingPluginOrder {
  Pre,
  Post,
}

impl From<BindingPluginOrder> for rolldown_plugin::PluginOrder {
  fn from(value: BindingPluginOrder) -> Self {
    match value {
      BindingPluginOrder::Pre => rolldown_plugin::PluginOrder::Pre,
      BindingPluginOrder::Post => rolldown_plugin::PluginOrder::Post,
    }
  }
}

#[napi(object, object_to_js = false)]
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingPluginHookMeta {
  pub order: Option<BindingPluginOrder>,
}

impl From<&BindingPluginHookMeta> for rolldown_plugin::PluginHookMeta {
  fn from(value: &BindingPluginHookMeta) -> Self {
    rolldown_plugin::PluginHookMeta { order: value.order.map(Into::into) }
  }
}
