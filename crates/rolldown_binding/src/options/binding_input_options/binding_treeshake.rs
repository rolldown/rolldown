use derive_more::Debug;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::{collections::HashSet, sync::Arc};

use napi::bindgen_prelude::{Either4, FnArgs};
use napi_derive::napi;
use rolldown::{InnerOptions, ModuleSideEffects, ModuleSideEffectsRule};
use rolldown_utils::js_regex::HybridRegex;

use crate::{
  types::js_callback::{JsCallback, JsCallbackExt},
  types::js_regex::JsRegExp,
};

#[napi]
#[derive(Debug)]
pub enum BindingPropertyReadSideEffects {
  Always,
  False,
}

#[napi]
#[derive(Debug)]
pub enum BindingPropertyWriteSideEffects {
  Always,
  False,
}

pub type BindingModuleSideEffects = Either4<
  bool,
  HashSet<String, FxBuildHasher>,
  Vec<BindingModuleSideEffectsRule>,
  JsCallback<FnArgs<(String, bool)>, Option<bool>>,
>;

#[napi_derive::napi(object, object_to_js = false)]
#[derive(Debug)]
pub struct BindingTreeshake {
  #[napi(
    ts_type = "boolean | ReadonlyArray<string> | BindingModuleSideEffectsRule[] | ((id: string, external: boolean) => boolean | undefined)"
  )]
  #[debug("ModuleSideEffects(...)")]
  pub module_side_effects: BindingModuleSideEffects,
  pub annotations: Option<bool>,
  #[napi(ts_type = "ReadonlyArray<string>")]
  pub manual_pure_functions: Option<FxHashSet<String>>,
  pub unknown_global_side_effects: Option<bool>,
  pub commonjs: Option<bool>,
  pub property_read_side_effects: Option<BindingPropertyReadSideEffects>,
  pub property_write_side_effects: Option<BindingPropertyWriteSideEffects>,
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
      Either4::A(value) => ModuleSideEffects::Boolean(value),
      Either4::B(rules) => ModuleSideEffects::IdSet(rules),
      Either4::C(rules) => {
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
      Either4::D(ts_fn) => {
        ModuleSideEffects::Function(Arc::new(move |id: &str, is_external: bool| {
          let id = id.to_string();
          let ts_fn = Arc::clone(&ts_fn);
          Box::pin(async move {
            ts_fn.invoke_async((id.clone(), is_external).into()).await.map_err(anyhow::Error::from)
          })
        }))
      }
    };

    let property_read_side_effects = value.property_read_side_effects.map(|v| match v {
      BindingPropertyReadSideEffects::Always => rolldown::PropertyReadSideEffects::Always,
      BindingPropertyReadSideEffects::False => rolldown::PropertyReadSideEffects::False,
    });

    let property_write_side_effects = value.property_write_side_effects.map(|v| match v {
      BindingPropertyWriteSideEffects::Always => rolldown::PropertyWriteSideEffects::Always,
      BindingPropertyWriteSideEffects::False => rolldown::PropertyWriteSideEffects::False,
    });

    Ok(Self::Option(InnerOptions {
      module_side_effects,
      annotations: value.annotations,
      manual_pure_functions: value.manual_pure_functions,
      unknown_global_side_effects: value.unknown_global_side_effects,
      // By default disable commonjs tree shake, since it is not stable
      commonjs: value.commonjs,
      property_read_side_effects,
      property_write_side_effects,
    }))
  }

  type Error = anyhow::Error;
}
