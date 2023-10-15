use crate::utils::napi_error_ext::NapiErrorExt;
use crate::utils::JsCallback;
use derivative::Derivative;
use rolldown_plugin::{BoxPlugin, Plugin, PluginName};

use super::plugin::{PluginOptions, ResolveIdResult, SourceResult};

pub type ResolveIdCallback = JsCallback<(String, Option<String>), Option<ResolveIdResult>>;
pub type LoadCallback = JsCallback<(String, Option<String>), Option<SourceResult>>;
pub type TransformCallback = JsCallback<(String, String), Option<SourceResult>>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct JsAdapterPlugin {
  pub name: String,
  #[derivative(Debug = "ignore")]
  resolve_id_fn: Option<ResolveIdCallback>,
  #[derivative(Debug = "ignore")]
  load_fn: Option<LoadCallback>,
  #[derivative(Debug = "ignore")]
  transform_fn: Option<TransformCallback>,
}

impl JsAdapterPlugin {
  pub fn new(option: PluginOptions) -> napi::Result<Self> {
    let resolve_id_fn = option
      .resolve_id
      .as_ref()
      .map(ResolveIdCallback::new)
      .transpose()?;
    let load_fn = option
      .resolve_id
      .as_ref()
      .map(LoadCallback::new)
      .transpose()?;
    let transform_fn = option
      .transform
      .as_ref()
      .map(TransformCallback::new)
      .transpose()?;
    Ok(JsAdapterPlugin {
      name: option.name,
      resolve_id_fn,
      load_fn,
      transform_fn,
    })
  }

  pub fn new_boxed(option: PluginOptions) -> napi::Result<BoxPlugin> {
    Ok(Box::new(Self::new(option)?))
  }
}

#[async_trait::async_trait]
impl Plugin for JsAdapterPlugin {
  fn name(&self) -> PluginName {
    std::borrow::Cow::Borrowed(&self.name)
  }

  async fn resolve_id(
    &self,
    _ctx: &mut rolldown_plugin::Context,
    args: &rolldown_plugin::ResolveIdArgs,
  ) -> rolldown_plugin::ResolveIdReturn {
    if let Some(cb) = &self.resolve_id_fn {
      let res = cb
        .call_async((
          args.source.to_string(),
          args.importer.map(|s| s.to_string()),
        ))
        .await
        .map_err(|e| e.into_bundle_error())?;

      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  async fn load(
    &self,
    _ctx: &mut rolldown_plugin::Context,
    args: &rolldown_plugin::LoadArgs,
  ) -> rolldown_plugin::LoadReturn {
    if let Some(cb) = &self.load_fn {
      let res = cb
        .call_async((args.id.to_string(), None))
        .await
        .map_err(|e| e.into_bundle_error())?;
      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  async fn transform(
    &self,
    _ctx: &mut rolldown_plugin::Context,
    args: &rolldown_plugin::TransformArgs,
  ) -> rolldown_plugin::TransformReturn {
    if let Some(cb) = &self.transform_fn {
      let res = cb
        .call_async((args.code.to_string(), args.id.to_string()))
        .await
        .map_err(|e| e.into_bundle_error())?;
      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }
}
