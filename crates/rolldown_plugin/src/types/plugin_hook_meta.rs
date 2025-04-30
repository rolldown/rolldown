#[derive(Debug, PartialEq, Eq)]
pub enum PluginOrder {
  Pre,
  Post,
}

#[derive(Debug)]
pub struct PluginHookMeta {
  pub order: Option<PluginOrder>,
}
