/// [`PluginContext`] itself will carry some general data for all hooks and a `context` field for
/// specific data for different hooks.
#[derive(Debug, Default)]
pub struct PluginContext<Ctx = ()> {
  /// The field is used to pass specific context for different hooks.
  pub context: Ctx,
}

impl PluginContext {
  pub fn new() -> Self {
    Self::with_context(())
  }
}

impl<T> PluginContext<T> {
  pub fn with_context(context: T) -> Self {
    Self { context }
  }
}
