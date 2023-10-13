use std::{borrow::Cow, fmt::Debug};

use crate::{Context, LoadArgs, ResolveIdArgs, ResolveIdResult, SourceResult, TransformArgs};

pub type ResolveIdReturn = rolldown_error::Result<Option<ResolveIdResult>>;
pub type TransformReturn = rolldown_error::Result<Option<SourceResult>>;
pub type LoadReturn = rolldown_error::Result<Option<SourceResult>>;
pub type PluginName<'a> = Cow<'a, str>;

#[async_trait::async_trait]
pub trait Plugin: Debug + Send + Sync {
  fn name(&self) -> PluginName;

  async fn resolve_id(&self, _ctx: &mut Context, _args: &ResolveIdArgs) -> ResolveIdReturn {
    Ok(None)
  }

  async fn load(&self, _ctx: &mut Context, _args: &LoadArgs) -> LoadReturn {
    Ok(None)
  }

  async fn transform(&self, _ctx: &mut Context, _args: &TransformArgs) -> TransformReturn {
    Ok(None)
  }
}

pub type BoxPlugin = Box<dyn Plugin>;
