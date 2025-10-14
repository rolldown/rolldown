use napi_derive::napi;

#[derive(Debug, Clone, Copy)]
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

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Default)]
pub struct BindingPluginHookMeta {
  pub order: Option<BindingPluginOrder>,
}

impl From<&BindingPluginHookMeta> for rolldown_plugin::PluginHookMeta {
  fn from(value: &BindingPluginHookMeta) -> Self {
    rolldown_plugin::PluginHookMeta { order: value.order.map(Into::into) }
  }
}
