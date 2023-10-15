#[derive(Debug, Default)]
pub struct Context<Ctx = ()> {
  pub context: Ctx,
}

impl Context {
  pub fn new() -> Self {
    Self::with_context(())
  }
}

impl<T> Context<T> {
  pub fn with_context(context: T) -> Self {
    Self { context }
  }
}
