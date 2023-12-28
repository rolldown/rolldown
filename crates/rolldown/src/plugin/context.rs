#[derive(Debug)]
pub struct PluginContext {}

impl PluginContext {
  pub fn new() -> Self {
    Self {}
  }

  pub fn load(&self) {}
}

#[derive(Debug)]
pub struct TransformPluginContext<'a> {
  pub inner: &'a PluginContext,
}

impl<'a> TransformPluginContext<'a> {
  pub fn new(inner: &'a PluginContext) -> Self {
    Self { inner }
  }
}
