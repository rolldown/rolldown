use std::{borrow::Cow, ops::Deref, sync::Arc};

use crate::utils::js_async_callback_ext::JsAsyncCallbackExt;
use futures::TryFutureExt;
use napi::bindgen_prelude::{Either, Either3, Error, Status};
use rolldown_plugin::Plugin;

use super::PluginOptions;

#[derive(Debug)]
pub struct PluginAdapter {
  pub(crate) inner: PluginOptions,
}

impl Deref for PluginAdapter {
  type Target = PluginOptions;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl PluginAdapter {
  pub(crate) fn new_boxed(inner: PluginOptions) -> Box<dyn Plugin> {
    Box::new(Self { inner })
  }
}

#[async_trait::async_trait]
impl Plugin for PluginAdapter {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::SharedPluginContext,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_start {
      cb.call_async_normalized(Arc::clone(ctx).into()).await?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookResolveIdArgs,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_id {
      let res = cb
        .call_async((
          args.source.to_string(),
          args.importer.map(|s| s.to_string()),
          Some(args.options.clone().into()),
        ))
        .and_then(|cb| async {
          match cb {
            Either3::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either3::B(result) => Ok(result),
            Either3::C(_) => {
              Err(Error::new(Status::InvalidArg, "Invalid return value from resolve_id hook"))
            }
          }
        })
        .await?;

      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn load(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookLoadArgs,
  ) -> rolldown_plugin::HookLoadReturn {
    if let Some(cb) = &self.load {
      let res = cb
        .call_async(args.id.to_string())
        .and_then(|loaded| async {
          match loaded {
            Either3::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either3::B(result) => Ok(result),
            Either3::C(_) => {
              Err(Error::new(Status::InvalidArg, "Invalid return value from load hook"))
            }
          }
        })
        .await?;
      Ok(res.map(|x| x.try_into()).transpose()?)
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn transform(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookTransformArgs,
  ) -> rolldown_plugin::HookTransformReturn {
    if let Some(cb) = &self.transform {
      let res = cb
        .call_async((args.code.to_string(), args.id.to_string()))
        .and_then(|transformed| async {
          match transformed {
            Either3::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either3::B(result) => Ok(result),
            Either3::C(_) => {
              Err(Error::new(Status::InvalidArg, "Invalid return value from transform hook"))
            }
          }
        })
        .await?;
      Ok(res.map(|x| x.try_into()).transpose()?)
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn build_end(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: Option<&rolldown_plugin::HookBuildEndArgs>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.build_end {
      cb.call_async(args.map(|a| a.error.to_string()))
        .and_then(|build_end| async {
          match build_end {
            Either::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either::B(_) => Ok(()),
          }
        })
        .await?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if let Some(cb) = &self.render_chunk {
      let res = cb
        .call_async((args.code.to_string(), args.chunk.clone().into()))
        .and_then(|rendered| async {
          match rendered {
            Either3::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either3::B(result) => Ok(result),
            Either3::C(_) => {
              Err(Error::new(Status::InvalidArg, "Invalid return value from render_chunk hook"))
            }
          }
        })
        .await?;
      return Ok(res.map(Into::into));
    }
    Ok(None)
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
    is_write: bool,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.generate_bundle {
      cb.call_async((bundle.clone().into(), is_write))
        .and_then(|generated| async {
          match generated {
            Either::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either::B(_) => Ok(()),
          }
        })
        .await?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn write_bundle(
    &self,
    _ctx: &rolldown_plugin::SharedPluginContext,
    bundle: &Vec<rolldown_common::Output>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(cb) = &self.write_bundle {
      cb.call_async(bundle.clone().into())
        .and_then(|written| async {
          match written {
            Either::A(p) => {
              let result = p.await?;
              Ok(result)
            }
            Either::B(_) => Ok(()),
          }
        })
        .await?;
    }
    Ok(())
  }
}
