use std::{borrow::Cow, fmt::Debug};

use super::{
  args::{HookLoadArgs, HookResolveIdArgs, HookTransformArgs},
  context::PluginContext,
  output::{HookLoadOutput, HookResolveIdOutput},
};

pub type HookResolveIdReturn = rolldown_error::BuildResult<Option<HookResolveIdOutput>>;
pub type HookTransformReturn = rolldown_error::BuildResult<Option<HookLoadOutput>>;
pub type HookLoadReturn = rolldown_error::BuildResult<Option<HookLoadOutput>>;

#[async_trait::async_trait]
pub trait Plugin: Debug + Send + Sync {
  fn name(&self) -> Cow<'static, str>;

  async fn resolve_id(
    &self,
    _ctx: &mut PluginContext,
    _args: &HookResolveIdArgs,
  ) -> HookResolveIdReturn {
    Ok(None)
  }

  async fn load(&self, _ctx: &mut PluginContext, _args: &HookLoadArgs) -> HookLoadReturn {
    Ok(None)
  }

  async fn transform(
    &self,
    _ctx: &mut PluginContext,
    _args: &HookTransformArgs,
  ) -> HookTransformReturn {
    Ok(None)
  }
}

pub type BoxPlugin = Box<dyn Plugin>;
