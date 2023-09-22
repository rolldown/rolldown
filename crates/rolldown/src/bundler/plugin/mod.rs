use std::fmt::Debug;

mod resolve_id;
mod util;
use crate::InputOptions;
mod plugin_driver;
use self::{
  resolve_id::{ResolveIdArgs, ResolveIdResult},
  util::SourceResult,
};

#[derive(Debug)]
pub struct ResolvedId {
  pub id: String,
  pub external: bool,
}

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

#[async_trait::async_trait]
pub trait Plugin: Debug + Send + Sync {
  fn name(&self) -> &'static str;

  async fn options(&self, _options: &mut InputOptions) -> rolldown_error::Result<()> {
    Ok(())
  }

  async fn build_start(&self, _options: &mut InputOptions) -> rolldown_error::Result<()> {
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &mut Context,
    _args: &ResolveIdArgs,
  ) -> rolldown_error::Result<Option<ResolveIdResult>> {
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &mut Context,
    _id: &str,
  ) -> rolldown_error::Result<Option<SourceResult>> {
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: &mut Context,
    _code: &str,
    _id: &str,
  ) -> rolldown_error::Result<Option<SourceResult>> {
    Ok(None)
  }

  async fn build_end(&self, _error: &rolldown_error::Error) -> rolldown_error::Result<()> {
    Ok(())
  }
}
