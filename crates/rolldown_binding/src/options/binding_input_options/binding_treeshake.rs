use std::{fmt::Debug, sync::Arc};

use napi::bindgen_prelude::Either3;
use rolldown::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule};
use rolldown_utils::js_regex::HybridRegex;

use crate::{
  options::plugin::types::binding_js_or_regex::JsRegExp,
  types::js_callback::{JsCallback, JsCallbackExt},
};

pub(crate) type BindingModuleSideEffects =
  Either3<bool, Vec<BindingModuleSideEffectsRule>, JsCallback<(String, bool), Option<bool>>>;

#[napi_derive::napi(object, object_to_js = false)]
pub struct BindingTreeshake {
  #[napi(
    ts_type = "boolean | BindingModuleSideEffectsRule[] | ((id: string, is_external: boolean) => boolean | undefined)"
  )]
  pub module_side_effects: BindingModuleSideEffects,
  pub annotations: Option<bool>,
}

impl Debug for BindingTreeshake {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingTreeshake")
      .field("module_side_effects", &"ModuleSideEffects")
      .field("annotations", &self.annotations)
      .finish()
  }
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug, Default)]
pub struct BindingModuleSideEffectsRule {
  #[napi(ts_type = "RegExp | undefined")]
  pub test: Option<JsRegExp>,
  pub side_effects: bool,
  #[napi(ts_type = "boolean | undefined")]
  pub external: Option<bool>,
}

impl TryFrom<BindingTreeshake> for rolldown::TreeshakeOptions {
  fn try_from(value: BindingTreeshake) -> anyhow::Result<Self> {
    let module_side_effects = match value.module_side_effects {
      Either3::A(value) => ModuleSideEffects::Boolean(value),
      Either3::B(rules) => {
        let mut ret = Vec::with_capacity(rules.len());
        for rule in rules {
          let test = match rule.test {
            Some(test) => Some(HybridRegex::try_from(test)?),
            None => None,
          };
          ret.push(ModuleSideEffectsRule {
            test,
            side_effects: rule.side_effects,
            external: rule.external,
          });
        }
        ModuleSideEffects::ModuleSideEffectsRules(ret)
      }
      Either3::C(ts_fn) => {
        ModuleSideEffects::Function(Arc::new(move |id: &str, is_external: bool| {
          let id = id.to_string();
          let ts_fn = Arc::clone(&ts_fn);
          Box::pin(async move {
            ts_fn.invoke_async((id.clone(), is_external)).await.map_err(anyhow::Error::from)
          })
        }))
      }
    };

    Ok(Self::Option(InnerOptions { module_side_effects, annotations: value.annotations }))
  }

  type Error = anyhow::Error;
}
