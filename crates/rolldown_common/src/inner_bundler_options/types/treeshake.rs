use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};

use derive_more::Debug;
use rolldown_utils::js_regex::HybridRegex;
use rustc_hash::FxHashSet;

use crate::{ResolvedId, side_effects::HookSideEffects};
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase")
)]
pub enum PropertyReadSideEffects {
  Always,
  False,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase")
)]
pub enum PropertyWriteSideEffects {
  Always,
  False,
}

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

  pub fn invalid_import_side_effects(&self) -> bool {
    self.as_ref().and_then(|item| item.invalid_import_side_effects).unwrap_or(false)
  }

  pub fn commonjs(&self) -> bool {
    self.as_ref().and_then(|item| item.commonjs).unwrap_or(true)
  }
  /// By default we assume property reads have side effects
  #[inline]
  pub fn property_read_side_effects(&self) -> PropertyReadSideEffects {
    self
      .as_ref()
      .and_then(|opt| opt.property_read_side_effects)
      .unwrap_or(PropertyReadSideEffects::Always)
  }

  /// By default we assume property writes have side effects
  #[inline]
  pub fn property_write_side_effects(&self) -> PropertyWriteSideEffects {
    self
      .as_ref()
      .and_then(|opt| opt.property_write_side_effects)
      .unwrap_or(PropertyWriteSideEffects::Always)
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
  #[debug("Rules({_0:?})")]
  Rules(Vec<ModuleSideEffectsRule>),
  #[debug("IdSet({_0:?})")]
  IdSet(FxHashSet<String>),
  #[debug("Boolean({_0})")]
  Boolean(bool),
  #[debug("Function")]
  Function(Arc<ModuleSideEffectsFn>),
}

type ModuleSideEffectsFn = dyn Fn(
    &str, // id
    bool, // external
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<bool>>> + Send + 'static>>
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
      ModuleSideEffects::Rules(rules) => {
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
      ModuleSideEffects::IdSet(set) => Some(set.contains(path)),
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
  pub property_read_side_effects: Option<PropertyReadSideEffects>,
  pub property_write_side_effects: Option<PropertyWriteSideEffects>,
  pub invalid_import_side_effects: Option<bool>,
}

impl Default for InnerOptions {
  fn default() -> Self {
    InnerOptions {
      module_side_effects: ModuleSideEffects::Boolean(true),
      annotations: Some(true),
      manual_pure_functions: None,
      unknown_global_side_effects: None,
      commonjs: None,
      property_read_side_effects: None,
      property_write_side_effects: None,
      invalid_import_side_effects: None,
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
      property_read_side_effects: match value.property_read_side_effects() {
        PropertyReadSideEffects::Always => oxc::minifier::PropertyReadSideEffects::All,
        PropertyReadSideEffects::False => oxc::minifier::PropertyReadSideEffects::None,
      },
      property_write_side_effects: match value.property_write_side_effects() {
        PropertyWriteSideEffects::Always => true,
        PropertyWriteSideEffects::False => false,
      },
      unknown_global_side_effects: value.unknown_global_side_effects(),
      invalid_import_side_effects: value.invalid_import_side_effects(),
    }
  }
}

fn hook_side_effects_from_bool(value: bool) -> HookSideEffects {
  if value { HookSideEffects::True } else { HookSideEffects::False }
}

/// Fallback once hook side effects and `treeshake.moduleSideEffects` have been applied.
/// Mirrors `lazy_check_side_effects`, with the analyzed-statement case reported as `true`.
fn reported_side_effects_fallback(
  resolved_id: &ResolvedId,
  check_package_json: bool,
) -> HookSideEffects {
  if resolved_id.external.is_external() {
    return if resolved_id.is_external_without_side_effects {
      HookSideEffects::False
    } else {
      HookSideEffects::NoTreeshake
    };
  }
  if check_package_json && let Some(value) = resolved_id.package_json_side_effects() {
    return hook_side_effects_from_bool(value);
  }
  HookSideEffects::True
}

/// `moduleSideEffects` value `this.resolve` should report for `resolved_id`.
///
/// Precedence matches module creation (`normalize_side_effects`): hook value, then
/// `treeshake.moduleSideEffects`, then `package.json#sideEffects`, then `true`.
/// Statement-level analysis is not part of the reported value. `Analyzed` modules
/// therefore show up as `true`. Keep this aligned with
/// [`crate::side_effects::DeterminedSideEffects::to_module_side_effects`].
pub async fn reported_module_side_effects(
  treeshake: &NormalizedTreeshakeOptions,
  resolved_id: &ResolvedId,
) -> anyhow::Result<HookSideEffects> {
  match resolved_id.side_effects {
    Some(HookSideEffects::False) => return Ok(HookSideEffects::False),
    Some(HookSideEffects::NoTreeshake) => return Ok(HookSideEffects::NoTreeshake),
    // `true` from a hook means "use the default algorithm", and it skips both the
    // treeshake option and `package.json#sideEffects`. Externals still take the
    // external short-circuit inside the fallback.
    Some(HookSideEffects::True) => return Ok(reported_side_effects_fallback(resolved_id, false)),
    None => {}
  }

  let Some(opt) = treeshake.as_ref() else {
    return Ok(HookSideEffects::NoTreeshake);
  };
  let from_option = if opt.module_side_effects.is_fn() {
    opt.module_side_effects.ffi_resolve(&resolved_id.id, resolved_id.external.is_external()).await?
  } else {
    opt.module_side_effects.native_resolve(&resolved_id.id, resolved_id.external.is_external())
  };
  if let Some(value) = from_option {
    return Ok(hook_side_effects_from_bool(value));
  }
  Ok(reported_side_effects_fallback(resolved_id, true))
}

#[cfg(test)]
mod reported_module_side_effects_tests {
  use std::{path::PathBuf, task::Context};

  use super::*;
  use crate::{PackageJson, side_effects::SideEffects};

  fn block_on_ready<T>(fut: impl Future<Output = T>) -> T {
    let mut fut = std::pin::pin!(fut);
    let waker = std::task::Waker::noop();
    let mut cx = Context::from_waker(waker);
    match fut.as_mut().poll(&mut cx) {
      std::task::Poll::Ready(value) => value,
      std::task::Poll::Pending => panic!("side-effect decision should not yield"),
    }
  }

  fn options(module_side_effects: ModuleSideEffects) -> NormalizedTreeshakeOptions {
    InnerOptions { module_side_effects, ..InnerOptions::default() }.into()
  }

  fn id_with_package_side_effects(side_effects: bool) -> ResolvedId {
    ResolvedId {
      id: "/pkg/a.js".into(),
      package_json: Some(Arc::new(PackageJson::with_side_effects_for_test(
        SideEffects::Bool(side_effects),
        PathBuf::from("/pkg/package.json"),
      ))),
      ..Default::default()
    }
  }

  #[test]
  fn reports_hook_then_option_then_package_json_then_default() {
    let disabled = NormalizedTreeshakeOptions::new(None);
    let mut hooked = id_with_package_side_effects(false);
    hooked.side_effects = Some(HookSideEffects::False);
    assert_eq!(
      block_on_ready(reported_module_side_effects(&disabled, &hooked)).unwrap(),
      HookSideEffects::False,
    );

    hooked.side_effects = Some(HookSideEffects::True);
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(false)),
        &hooked
      ))
      .unwrap(),
      HookSideEffects::True,
    );

    let plain = id_with_package_side_effects(false);
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(false)),
        &plain
      ))
      .unwrap(),
      HookSideEffects::False,
    );
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(true)),
        &plain
      ))
      .unwrap(),
      HookSideEffects::False,
    );

    let mut bare = ResolvedId { id: "/src/index.js".into(), ..Default::default() };
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(true)),
        &bare
      ))
      .unwrap(),
      HookSideEffects::True,
    );
    assert_eq!(
      block_on_ready(reported_module_side_effects(&disabled, &bare)).unwrap(),
      HookSideEffects::NoTreeshake,
    );

    bare.external = true.into();
    bare.is_external_without_side_effects = true;
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(true)),
        &bare
      ))
      .unwrap(),
      HookSideEffects::False,
    );
    bare.is_external_without_side_effects = false;
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(true)),
        &bare
      ))
      .unwrap(),
      HookSideEffects::NoTreeshake,
    );
  }
}
