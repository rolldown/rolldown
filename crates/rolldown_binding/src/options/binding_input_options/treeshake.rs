use napi::Either;
use rolldown::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule};
use rolldown_utils::js_regex::HybridRegex;
use serde::Deserialize;

use crate::options::plugin::types::binding_js_or_regex::JsRegExp;

pub(crate) type BindingModuleSideEffects = Either<bool, Vec<BindingModuleSideEffectsRule>>;
#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BindingTreeshake {
  #[napi(ts_type = "boolean | BindingModuleSideEffectsRule[]")]
  #[serde(skip_deserializing, default = "default_module_side_effects")]
  pub module_side_effects: BindingModuleSideEffects,
}

fn default_module_side_effects() -> BindingModuleSideEffects {
  Either::A(true)
}

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct BindingModuleSideEffectsRule {
  #[napi(ts_type = "RegExp | undefined")]
  pub test: Option<JsRegExp>,
  pub side_effects: bool,
  #[napi(ts_type = "boolean | undefined")]
  pub external: Option<bool>,
}

impl TryFrom<BindingTreeshake> for rolldown::TreeshakeOptions {
  fn try_from(value: BindingTreeshake) -> anyhow::Result<Self> {
    match value.module_side_effects {
      Either::A(value) => {
        Ok(Self::Option(InnerOptions { module_side_effects: ModuleSideEffects::Boolean(value) }))
      }
      Either::B(rules) => {
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

        Ok(Self::Option(InnerOptions {
          module_side_effects: ModuleSideEffects::ModuleSideEffectsRules(ret),
        }))
      }
    }
  }

  type Error = anyhow::Error;
}
