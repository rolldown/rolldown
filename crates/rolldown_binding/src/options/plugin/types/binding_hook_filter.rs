use rolldown::ModuleType;
use rolldown_plugin::{GeneralHookFilter, ResolvedIdHookFilter, TransformHookFilter};

use super::binding_js_or_regex::{bindingify_string_or_regex_array, BindingStringOrRegex};

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Clone, Debug)]
pub struct BindingGeneralHookFilter {
  pub include: Option<Vec<BindingStringOrRegex>>,
  pub exclude: Option<Vec<BindingStringOrRegex>>,
}

impl TryFrom<BindingGeneralHookFilter> for GeneralHookFilter {
  type Error = anyhow::Error;

  fn try_from(value: BindingGeneralHookFilter) -> Result<Self, Self::Error> {
    Ok(Self {
      include: value.include.map(bindingify_string_or_regex_array).transpose()?,
      exclude: value.exclude.map(bindingify_string_or_regex_array).transpose()?,
    })
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
#[derive(Default, Clone)]
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
