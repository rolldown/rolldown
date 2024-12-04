use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(untagged)
)]
pub enum TreeshakeOptions {
  Boolean(bool),
  Option(InnerOptions),
}

impl Default for TreeshakeOptions {
  /// Used for snapshot testing
  fn default() -> Self {
    TreeshakeOptions::Option(InnerOptions {
      module_side_effects: ModuleSideEffects::Boolean(true),
      annotations: Some(true),
    })
  }
}

#[derive(Clone)]
pub enum ModuleSideEffects {
  ModuleSideEffectsRules(Vec<ModuleSideEffectsRule>),
  Boolean(bool),
  Function(Arc<ModuleSideEffectsFn>),
}

impl Debug for ModuleSideEffects {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      ModuleSideEffects::ModuleSideEffectsRules(rules) => {
        f.debug_tuple("ModuleSideEffectsRules").field(rules).finish()
      }
      ModuleSideEffects::Boolean(v) => f.debug_tuple("Boolean").field(v).finish(),
      ModuleSideEffects::Function(_) => f.write_str("Function"),
    }
  }
}

type ModuleSideEffectsFn = dyn Fn(
    &str, // id
    bool, // is_resolved
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<bool>>> + Send + 'static)>>
  + Send
  + Sync
  + 'static;

#[derive(Debug, Clone)]
pub struct ModuleSideEffectsRule {
  pub test: Option<HybridRegex>,
  pub external: Option<bool>,
  pub side_effects: bool,
}

impl ModuleSideEffects {
  pub fn is_fn(&self) -> bool {
    matches!(self, ModuleSideEffects::Function(_))
  }

  /// # Panic
  /// Panics if the side effects are defined as a function
  pub fn native_resolve(&self, path: &str, is_external: bool) -> Option<bool> {
    match self {
      ModuleSideEffects::ModuleSideEffectsRules(rules) => {
        for ModuleSideEffectsRule { test, external, side_effects } in rules {
          match (test, external) {
            (Some(test), Some(external)) => {
              if test.matches(path) && *external == is_external {
                return Some(*side_effects);
              }
            }
            (None, Some(external)) => {
              if *external == is_external {
                return Some(*side_effects);
              }
            }
            (Some(test), None) => {
              if test.matches(path) {
                return Some(*side_effects);
              }
            }
            // At least one of `test` or `external` should be defined
            (None, None) => unreachable!(),
          };
        }
        // analyze side effects from source code
        None
      }
      ModuleSideEffects::Boolean(false) => Some(false),
      ModuleSideEffects::Boolean(true) => None,
      ModuleSideEffects::Function(_) => unreachable!(),
    }
  }

  /// resolve the side effects from the ffi function
  /// # Panic
  /// Panics if the side effects are not defined as a function
  pub async fn ffi_resolve(&self, path: &str, is_external: bool) -> anyhow::Result<Option<bool>> {
    match self {
      ModuleSideEffects::Function(f) => Ok(f(path, is_external).await?),
      _ => unreachable!(),
    }
  }
}

impl TreeshakeOptions {
  pub fn enabled(&self) -> bool {
    matches!(self, TreeshakeOptions::Option(_))
  }
  pub fn annotations(&self) -> bool {
    match self {
      TreeshakeOptions::Boolean(v) => *v,
      TreeshakeOptions::Option(inner) => inner.annotations.unwrap_or_default(),
    }
  }
}

#[derive(Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct InnerOptions {
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_module_side_effects"),
    schemars(with = "Option<bool>")
  )]
  pub module_side_effects: ModuleSideEffects,
  pub annotations: Option<bool>,
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_module_side_effects<'de, D>(deserializer: D) -> Result<ModuleSideEffects, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<bool>::deserialize(deserializer)?;
  match deserialized {
    Some(false) => Ok(ModuleSideEffects::Boolean(false)),
    Some(true) | None => Ok(ModuleSideEffects::Boolean(true)),
  }
}
