use std::borrow::Cow;

use crate::utils::globs::{create_glob_with_star_prefix, create_globset};
use crate::utils::napi_error_ext::NapiErrorExt;
use crate::utils::JsCallback;
use derivative::Derivative;
use once_cell::sync::OnceCell;
use rolldown::Plugin;
use rolldown_error::BuildError;
use wax::Pattern;

use super::plugin::{
  FilterIdResult, FilterIdResultGlobs, HookResolveIdArgsOptions, PluginOptions, ResolveIdResult,
  SourceResult,
};

pub type BuildStartCallback = JsCallback<(), ()>;
pub type ResolveIdCallback =
  JsCallback<(String, Option<String>, HookResolveIdArgsOptions), Option<ResolveIdResult>>;
pub type FilterIdCallback = JsCallback<(), FilterIdResult>;
pub type LoadCallback = JsCallback<(String,), Option<SourceResult>>;
pub type TransformCallback = JsCallback<(String, String), Option<SourceResult>>;
pub type BuildEndCallback = JsCallback<(Option<String>,), ()>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct JsAdapterPlugin {
  pub name: String,
  #[derivative(Debug = "ignore")]
  build_start_fn: Option<BuildStartCallback>,
  #[derivative(Debug = "ignore")]
  resolve_id_fn: Option<ResolveIdCallback>,
  #[derivative(Debug = "ignore")]
  filter_id_fn: Option<FilterIdCallback>,
  #[derivative(Debug = "ignore")]
  load_fn: Option<LoadCallback>,
  #[derivative(Debug = "ignore")]
  transform_fn: Option<TransformCallback>,
  #[derivative(Debug = "ignore")]
  build_end_fn: Option<BuildEndCallback>,

  filters_cache: OnceCell<FilterIdResultGlobs>,
}

impl JsAdapterPlugin {
  pub fn new(option: PluginOptions) -> napi::Result<Self> {
    let build_start_fn = option.build_start.as_ref().map(BuildStartCallback::new).transpose()?;
    let resolve_id_fn = option.resolve_id.as_ref().map(ResolveIdCallback::new).transpose()?;
    let filter_id_fn = option.filter_id.as_ref().map(FilterIdCallback::new).transpose()?;
    let load_fn = option.load.as_ref().map(LoadCallback::new).transpose()?;
    let transform_fn = option.transform.as_ref().map(TransformCallback::new).transpose()?;
    let build_end_fn = option.build_end.as_ref().map(BuildEndCallback::new).transpose()?;

    Ok(Self {
      name: option.name,
      build_start_fn,
      resolve_id_fn,
      filter_id_fn,
      load_fn,
      transform_fn,
      build_end_fn,
      filters_cache: OnceCell::new(),
    })
  }

  pub fn new_boxed(option: PluginOptions) -> napi::Result<Box<dyn Plugin>> {
    Ok(Box::new(Self::new(option)?))
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  pub async fn load_filters(&self) -> Result<&FilterIdResultGlobs, BuildError> {
    if let Some(filters) = self.filters_cache.get() {
      return Ok(filters);
    }

    let result = if let Some(cb) = &self.filter_id_fn {
      cb.call_async(()).await.map_err(|e| e.into_bundle_error())?
    } else {
      FilterIdResult::default()
    };

    let mut include = vec![];
    let mut exclude = vec![];

    for pattern in result.include {
      include.push(
        create_glob_with_star_prefix(&pattern)
          .map_err(|error| BuildError::invalid_glob(pattern, error))?,
      );
    }

    for pattern in result.exclude {
      exclude.push(
        create_glob_with_star_prefix(&pattern)
          .map_err(|error| BuildError::invalid_glob(pattern, error))?,
      );
    }

    self
      .filters_cache
      .set(FilterIdResultGlobs {
        include: if include.is_empty() {
          None
        } else {
          Some(create_globset(include).map_err(BuildError::invalid_globset)?)
        },
        exclude: if exclude.is_empty() {
          None
        } else {
          Some(create_globset(exclude).map_err(BuildError::invalid_globset)?)
        },
      })
      .unwrap();

    Ok(self.filters_cache.get().unwrap())
  }
}

#[async_trait::async_trait]
impl Plugin for JsAdapterPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.to_string())
  }

  // Returns true if the ID matches the list of filters (or if there are no filters).
  // TODO: apply to shouldTransformCachedModule, moduleParsed, watchChange
  async fn is_id_allowed(&self, id: &str) -> rolldown::HookBoolReturn {
    let filters = self.load_filters().await?;

    if let Some(exclude) = &filters.exclude {
      if exclude.is_match(id) {
        return Ok(false);
      }
    }

    Ok(filters.include.as_ref().map_or(true, |include| include.is_match(id)))
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn build_start(&self, _ctx: &mut rolldown::PluginContext) -> rolldown::HookNoopReturn {
    if let Some(cb) = &self.build_start_fn {
      cb.call_async(()).await.map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn resolve_id(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookResolveIdArgs,
  ) -> rolldown::HookResolveIdReturn {
    if let Some(cb) = &self.resolve_id_fn {
      let res = cb
        .call_async((
          args.source.to_string(),
          args.importer.map(|s| s.to_string()),
          args.options.clone().into(),
        ))
        .await
        .map_err(|e| e.into_bundle_error())?;

      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn load(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookLoadArgs,
  ) -> rolldown::HookLoadReturn {
    if !self.is_id_allowed(args.id).await? {
      return Ok(None);
    }

    if let Some(cb) = &self.load_fn {
      let res = cb.call_async((args.id.to_string(),)).await.map_err(|e| e.into_bundle_error())?;
      Ok(res.map(Into::into))
    } else {
      Ok(None)
    }
  }

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn transform(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: &rolldown::HookTransformArgs,
  ) -> rolldown::HookTransformReturn {
    if !self.is_id_allowed(args.id).await? {
      return Ok(None);
    }

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

  #[allow(clippy::redundant_closure_for_method_calls)]
  async fn build_end(
    &self,
    _ctx: &mut rolldown::PluginContext,
    args: Option<&rolldown::HookBuildEndArgs>,
  ) -> rolldown::HookNoopReturn {
    if let Some(cb) = &self.build_end_fn {
      cb.call_async((args.map(|a| a.error.to_string()),))
        .await
        .map_err(|e| e.into_bundle_error())?;
    }
    Ok(())
  }
}
