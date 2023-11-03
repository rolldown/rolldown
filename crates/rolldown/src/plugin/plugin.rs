use std::{borrow::Cow, fmt::Debug};

use crate::error::BatchedResult;

use super::{
  args::{HookLoadArgs, HookResolveIdArgs, HookTransformArgs},
  context::PluginContext,
  output::{HookLoadOutput, HookResolveIdOutput},
};

pub type HookResolveIdReturn = BatchedResult<Option<HookResolveIdOutput>>;
pub type HookTransformReturn = BatchedResult<Option<HookLoadOutput>>;
pub type HookLoadReturn = BatchedResult<Option<HookLoadOutput>>;

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
