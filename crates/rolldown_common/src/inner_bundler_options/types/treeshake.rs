use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug)]
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
    TreeshakeOptions::Option(InnerOptions { module_side_effects: ModuleSideEffects::Boolean(true) })
  }
}

#[derive(Debug)]
pub enum ModuleSideEffects {
  ModuleSideEffectsRules(Vec<ModuleSideEffectsRule>),
  Boolean(bool),
}

#[derive(Debug)]
pub struct ModuleSideEffectsRule {
  test: Option<HybridRegex>,
  external: Option<bool>,
  side_effects: bool,
}

impl ModuleSideEffects {
  pub fn resolve(&self, path: &str, is_external: bool) -> Option<bool> {
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
            (None, None) => unreachable!(),
          };
        }
        // analyze side effects from source code
        None
      }
      ModuleSideEffects::Boolean(b) => Some(*b),
    }
  }
}

impl TreeshakeOptions {
  pub fn enabled(&self) -> bool {
    matches!(self, TreeshakeOptions::Option(_))
  }
}

#[derive(Debug)]
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
