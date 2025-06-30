use derive_more::Debug;
use rustc_hash::FxHashSet;
use std::sync::Arc;

use napi::bindgen_prelude::{Either3, FnArgs};
use rolldown::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule};
use rolldown_utils::js_regex::HybridRegex;

use crate::{
  types::js_callback::{JsCallback, JsCallbackExt},
  types::js_regex::JsRegExp,
};

pub type BindingModuleSideEffects = Either3<
  bool,
  Vec<BindingModuleSideEffectsRule>,
  JsCallback<FnArgs<(String, bool)>, Option<bool>>,
>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingTreeshake {
  #[napi(
    ts_type = "boolean | BindingModuleSideEffectsRule[] | ((id: string, is_external: boolean) => boolean | undefined)"
  )]
  #[debug("ModuleSideEffects(...)")]
  pub module_side_effects: BindingModuleSideEffects,
  pub annotations: Option<bool>,
  pub manual_pure_functions: Option<Vec<String>>,
  pub unknown_global_side_effects: Option<bool>,
  pub commonjs: Option<bool>,
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
            ts_fn.invoke_async((id.clone(), is_external).into()).await.map_err(anyhow::Error::from)
          })
        }))
      }
    };

    Ok(Self::Option(InnerOptions {
      module_side_effects,
      annotations: value.annotations,
      manual_pure_functions: value.manual_pure_functions.map(FxHashSet::from_iter),
      unknown_global_side_effects: value.unknown_global_side_effects,
      // By default disable commonjs tree shake, since it is not stable
      commonjs: value.commonjs,
    }))
  }

  type Error = anyhow::Error;
}
