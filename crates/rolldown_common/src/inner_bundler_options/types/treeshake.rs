use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};

use derive_more::Debug;
use rolldown_utils::js_regex::HybridRegex;
use rustc_hash::FxHashSet;
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

#[derive(Default, Debug)]
pub struct NormalizedTreeshakeOptions(Option<InnerOptions>);

impl Deref for NormalizedTreeshakeOptions {
  type Target = Option<InnerOptions>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
impl From<InnerOptions> for NormalizedTreeshakeOptions {
  fn from(inner: InnerOptions) -> Self {
    NormalizedTreeshakeOptions(Some(inner))
  }
}

impl NormalizedTreeshakeOptions {
  pub fn new(inner: Option<InnerOptions>) -> Self {
    NormalizedTreeshakeOptions(inner)
  }

  pub fn annotations(&self) -> bool {
    self.as_ref().and_then(|item| item.annotations).unwrap_or(true)
  }

  pub fn unknown_global_side_effects(&self) -> bool {
    self.as_ref().and_then(|item| item.unknown_global_side_effects).unwrap_or(true)
  }

  pub fn commonjs(&self) -> bool {
    self.as_ref().and_then(|item| item.commonjs).unwrap_or(false)
  }

  // TODO: optimize this
  pub fn manual_pure_functions(&self) -> Option<&FxHashSet<String>> {
    self.as_ref().and_then(|item| item.manual_pure_functions.as_ref())
  }
}

impl Default for TreeshakeOptions {
  /// Used for snapshot testing
  fn default() -> Self {
    TreeshakeOptions::Option(InnerOptions::default())
  }
}

#[derive(Clone, Debug)]
pub enum ModuleSideEffects {
  #[debug("ModuleSideEffectsRules({_0:?})")]
  ModuleSideEffectsRules(Vec<ModuleSideEffectsRule>),
  #[debug("Boolean({_0})")]
  Boolean(bool),
  #[debug("Function")]
  Function(Arc<ModuleSideEffectsFn>),
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
          let is_match_rule = match (test, external) {
            (Some(test), Some(external)) => test.matches(path) && *external == is_external,
            (None, Some(external)) => *external == is_external,
            (Some(test), None) => test.matches(path),
            // At least one of `test` or `external` should be defined
            (None, None) => unreachable!(),
          };
          if is_match_rule {
            return Some(*side_effects);
          }
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

  pub fn into_normalized_options(self) -> NormalizedTreeshakeOptions {
    match self {
      TreeshakeOptions::Boolean(true) => InnerOptions::default().into(),
      TreeshakeOptions::Boolean(false) => NormalizedTreeshakeOptions::new(None),
      TreeshakeOptions::Option(inner_options) => inner_options.into(),
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
  pub manual_pure_functions: Option<FxHashSet<String>>,
  pub unknown_global_side_effects: Option<bool>,
  pub commonjs: Option<bool>,
}

impl Default for InnerOptions {
  fn default() -> Self {
    InnerOptions {
      module_side_effects: ModuleSideEffects::Boolean(true),
      annotations: Some(true),
      manual_pure_functions: None,
      unknown_global_side_effects: None,
      commonjs: None,
    }
  }
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

impl From<&NormalizedTreeshakeOptions> for oxc::minifier::TreeShakeOptions {
  fn from(value: &NormalizedTreeshakeOptions) -> Self {
    let default = oxc::minifier::TreeShakeOptions::default();
    oxc::minifier::TreeShakeOptions {
      annotations: value.annotations(),
      manual_pure_functions: value
        .manual_pure_functions()
        .map_or(default.manual_pure_functions, |set| set.iter().cloned().collect::<Vec<_>>()),
      property_read_side_effects: default.property_read_side_effects,
      unknown_global_side_effects: value.unknown_global_side_effects(),
    }
  }
}
