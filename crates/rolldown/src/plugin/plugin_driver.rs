use std::sync::Arc;

use tokio::sync::RwLock;

use crate::InputOptions;

use super::{
  resolve_id::{ResolveIdArgs, ResolveIdResult},
  util::SourceResult,
  Context, Plugin,
};

pub(crate) type SharedPluginDriver = Arc<RwLock<PluginDriver>>;

#[derive(Debug, Default)]
pub struct PluginDriver {
  pub plugins: Vec<Box<dyn Plugin>>,
}

impl PluginDriver {
  pub fn new(plugins: Vec<Box<dyn Plugin>>) -> Self {
    Self { plugins }
  }

  pub fn into_shared(self) -> SharedPluginDriver {
    Arc::new(RwLock::new(self))
  }

  pub async fn options(&self, options: &mut InputOptions) -> rolldown_error::Result<()> {
    for plugin in &self.plugins {
      plugin.options(options).await?;
    }
    Ok(())
  }

  pub async fn build_start(&self, options: &mut InputOptions) -> rolldown_error::Result<()> {
    for plugin in &self.plugins {
      plugin.build_start(options).await?;
    }
    Ok(())
  }

  pub async fn resolve_id(
    &self,
    args: &ResolveIdArgs<'_>,
  ) -> rolldown_error::Result<Option<ResolveIdResult>> {
    for plugin in &self.plugins {
      if let Some(result) = plugin.resolve_id(&mut Context::new(), args).await? {
        return Ok(Some(result));
      }
    }
    Ok(None)
  }

  pub async fn load(&self, id: &str) -> rolldown_error::Result<Option<SourceResult>> {
    for plugin in &self.plugins {
      if let Some(result) = plugin.load(&mut Context::new(), id).await? {
        return Ok(Some(result));
      }
    }
    Ok(None)
  }

  pub(crate) async fn transform(
    &self,
    code: &str,
    id: &str,
  ) -> rolldown_error::Result<Option<SourceResult>> {
    for plugin in &self.plugins {
      if let Some(result) = plugin.transform(&mut Context::new(), code, id).await? {
        return Ok(Some(result));
      }
    }
    Ok(None)
  }

  pub async fn build_end(&self, error: &rolldown_error::Error) -> rolldown_error::Result<()> {
    for plugin in &self.plugins {
      plugin.build_end(error).await?;
    }
    Ok(())
  }
}
