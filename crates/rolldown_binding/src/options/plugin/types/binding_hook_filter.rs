use rolldown::ModuleType;
use rolldown_plugin::{GeneralHookFilter, ResolvedIdHookFilter, TransformHookFilter};
use serde::Deserialize;

use super::binding_js_or_regex::BindingStringOrRegex;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BindingGeneralHookFilter {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl TryFrom<BindingGeneralHookFilter> for GeneralHookFilter {
  type Error = anyhow::Error;

  fn try_from(value: BindingGeneralHookFilter) -> Result<Self, Self::Error> {
    let mut ret = Self::default();
    if let Some(binding_include) = value.include {
      let mut include = Vec::with_capacity(binding_include.len());
      for i in binding_include {
        include.push(i.try_into()?);
      }
      ret.include = Some(include);
    }
    if let Some(binding_exclude) = value.exclude {
      let mut exclude = vec![];
      for i in binding_exclude {
        exclude.push(i.try_into()?);
      }
      ret.exclude = Some(exclude);
    }
    Ok(ret)
  }
}

impl TryFrom<BindingGeneralHookFilter> for ResolvedIdHookFilter {
  type Error = anyhow::Error;

  fn try_from(value: BindingGeneralHookFilter) -> Result<Self, Self::Error> {
    let mut ret = Self::default();
    let id_filter = value.try_into()?;
    ret.id = Some(id_filter);
    Ok(ret)
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BindingTransformHookFilter {
  pub code: Option<BindingGeneralHookFilter>,
  pub module_type: Option<Vec<String>>,
  pub id: Option<BindingGeneralHookFilter>,
}

impl TryFrom<BindingTransformHookFilter> for TransformHookFilter {
  type Error = anyhow::Error;

  fn try_from(value: BindingTransformHookFilter) -> Result<Self, Self::Error> {
    let mut default = Self::default();
    if let Some(code_filter) = value.code {
      let ret = code_filter.try_into()?;
      default.code = Some(ret);
    }
    if let Some(id_filter) = value.id {
      let ret = id_filter.try_into()?;
      default.id = Some(ret);
    }
    if let Some(module_type) = value.module_type {
      default.module_type =
        Some(module_type.into_iter().map(ModuleType::from_str_with_fallback).collect());
    }
    Ok(default)
  }
}
