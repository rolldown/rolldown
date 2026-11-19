use std::{future::Future, ops::Deref, pin::Pin, sync::Arc};

use derive_more::Debug;
use rolldown_utils::js_regex::HybridRegex;
use rustc_hash::FxHashSet;

use crate::{
  ResolvedId,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};
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

fn defined(value: bool) -> ModuleSideEffectsVerdict {
  ModuleSideEffectsVerdict::Final {
    determined: DeterminedSideEffects::UserDefined(value),
    reported: hook_side_effects_from_bool(value),
  }
}

/// Inclusion verdict and the `moduleSideEffects` value plugins see, before statement analysis.
///
/// `normalize_side_effects` and [`reported_module_side_effects`] both use this so the
/// order cannot drift. `treeshake.moduleSideEffects` is consulted with [`crate::ModuleId::raw_id`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleSideEffectsVerdict {
  /// A hook, `treeshake.moduleSideEffects`, `package.json#sideEffects`, or the external /
  /// `treeshake: false` fallback settled both values.
  Final { determined: DeterminedSideEffects, reported: HookSideEffects },
  /// Statements still need to be analyzed. Plugins report `true`.
  Analyze,
}

/// Shared side-effect order for inclusion and for `getModuleInfo` / `this.resolve`.
///
/// 1. Hook side effects. `false` and `no-treeshake` are final. `true` means the
///    default algorithm: it skips `treeshake.moduleSideEffects` and
///    `package.json#sideEffects`, except when `treeshake` is `false`, where `true`
///    counts as no value.
/// 2. `treeshake: false` (no hook value, or a hook `true`) includes the module and
///    reports `true`.
/// 3. `treeshake.moduleSideEffects`, passed the id's [`raw_id`](crate::ModuleId::raw_id).
/// 4. Externals with no hook value. A pure external (`is_external_without_side_effects`)
///    is `false`. Any other external stays included and reports `true`.
/// 5. `package.json#sideEffects`.
/// 6. [`ModuleSideEffectsVerdict::Analyze`]. Plugins report `true`.
///
/// An explicit hook `no-treeshake` stays `no-treeshake`. Hook `true` on an external still
/// uses the external short-circuit when treeshake is on, including `no-treeshake` when the
/// external is not pure. With treeshake off, hook `true` counts as no value.
pub async fn module_side_effects_decision(
  treeshake: &NormalizedTreeshakeOptions,
  resolved_id: &ResolvedId,
  mut hook_side_effects: Option<HookSideEffects>,
) -> anyhow::Result<ModuleSideEffectsVerdict> {
  // `this.resolve()` reports the effective `true`. A plugin that returns that object
  // (`@rollup/plugin-alias`) would otherwise turn statement analysis on and drop the import.
  if treeshake.is_none() && hook_side_effects == Some(HookSideEffects::True) {
    hook_side_effects = None;
  }
  match hook_side_effects {
    Some(HookSideEffects::False) => return Ok(defined(false)),
    Some(HookSideEffects::NoTreeshake) => {
      return Ok(ModuleSideEffectsVerdict::Final {
        determined: DeterminedSideEffects::NoTreeshake,
        reported: HookSideEffects::NoTreeshake,
      });
    }
    // `true` from a hook means "use the default algorithm".
    Some(HookSideEffects::True) => {
      return Ok(if resolved_id.external.is_external() {
        external_without_option(resolved_id, true)
      } else {
        ModuleSideEffectsVerdict::Analyze
      });
    }
    None => {}
  }

  // Rollup uses `() => true` when `treeshake` is off (`ModuleLoader` constructor).
  // Inclusion stays `NoTreeshake`, which is what disables statement treeshaking.
  let Some(opt) = treeshake.as_ref() else {
    return Ok(ModuleSideEffectsVerdict::Final {
      determined: DeterminedSideEffects::NoTreeshake,
      reported: HookSideEffects::True,
    });
  };

  let is_external = resolved_id.external.is_external();
  let from_option = if opt.module_side_effects.is_fn() {
    opt.module_side_effects.ffi_resolve(resolved_id.id.raw_id(), is_external).await?
  } else {
    opt.module_side_effects.native_resolve(resolved_id.id.raw_id(), is_external)
  };
  if let Some(value) = from_option {
    return Ok(defined(value));
  }
  if is_external {
    return Ok(external_without_option(resolved_id, false));
  }
  if let Some(value) = resolved_id.package_json_side_effects() {
    return Ok(defined(value));
  }
  Ok(ModuleSideEffectsVerdict::Analyze)
}

/// External that `treeshake.moduleSideEffects` did not settle.
///
/// `hook_said_true` keeps the historical report of `no-treeshake`. With no hook value,
/// Rollup's default `moduleSideEffects` reports `true` (`getResolvedIdWithDefaults`).
/// Either way the module stays included, unless it is a pure external.
fn external_without_option(
  resolved_id: &ResolvedId,
  hook_said_true: bool,
) -> ModuleSideEffectsVerdict {
  if resolved_id.is_external_without_side_effects {
    return defined(false);
  }
  ModuleSideEffectsVerdict::Final {
    determined: DeterminedSideEffects::NoTreeshake,
    reported: if hook_said_true { HookSideEffects::NoTreeshake } else { HookSideEffects::True },
  }
}

/// `moduleSideEffects` value `this.resolve` should report for `resolved_id`.
///
/// Uses [`module_side_effects_decision`]. Statement-level analysis is reported as `true`.
pub async fn reported_module_side_effects(
  treeshake: &NormalizedTreeshakeOptions,
  resolved_id: &ResolvedId,
) -> anyhow::Result<HookSideEffects> {
  Ok(match module_side_effects_decision(treeshake, resolved_id, resolved_id.side_effects).await? {
    ModuleSideEffectsVerdict::Final { reported, .. } => reported,
    ModuleSideEffectsVerdict::Analyze => HookSideEffects::True,
  })
}

#[cfg(test)]
mod reported_module_side_effects_tests {
  use std::{future::Future, path::PathBuf, task::Context};

  use rustc_hash::FxHashSet;

  use super::*;
  use crate::{ImportAttributes, ModuleId, PackageJson, side_effects::SideEffects};

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
    // `treeshake: false` with no hook value reports `true`. Inclusion stays `NoTreeshake`.
    assert_eq!(
      block_on_ready(module_side_effects_decision(&disabled, &bare, None)).unwrap(),
      ModuleSideEffectsVerdict::Final {
        determined: DeterminedSideEffects::NoTreeshake,
        reported: HookSideEffects::True,
      },
    );
    // A hook `true` counts as no value when treeshake is off. It must not request
    // statement analysis. Plugins still see the effective `true`.
    assert_eq!(
      block_on_ready(module_side_effects_decision(&disabled, &bare, Some(HookSideEffects::True)))
        .unwrap(),
      ModuleSideEffectsVerdict::Final {
        determined: DeterminedSideEffects::NoTreeshake,
        reported: HookSideEffects::True,
      },
    );
    bare.side_effects = Some(HookSideEffects::True);
    assert_eq!(
      block_on_ready(reported_module_side_effects(&disabled, &bare)).unwrap(),
      HookSideEffects::True,
    );
    bare.side_effects = None;
    bare.side_effects = Some(HookSideEffects::NoTreeshake);
    assert_eq!(
      block_on_ready(reported_module_side_effects(&disabled, &bare)).unwrap(),
      HookSideEffects::NoTreeshake,
    );
    bare.side_effects = None;

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
    // No hook value: a normal external reports `true` and stays included.
    bare.is_external_without_side_effects = false;
    assert_eq!(
      block_on_ready(module_side_effects_decision(
        &options(ModuleSideEffects::Boolean(true)),
        &bare,
        None
      ))
      .unwrap(),
      ModuleSideEffectsVerdict::Final {
        determined: DeterminedSideEffects::NoTreeshake,
        reported: HookSideEffects::True,
      },
    );
    // Hook `true` still takes the external short-circuit and reports `no-treeshake`.
    assert_eq!(
      block_on_ready(module_side_effects_decision(
        &options(ModuleSideEffects::Boolean(true)),
        &bare,
        Some(HookSideEffects::True)
      ))
      .unwrap(),
      ModuleSideEffectsVerdict::Final {
        determined: DeterminedSideEffects::NoTreeshake,
        reported: HookSideEffects::NoTreeshake,
      },
    );
    // An explicit option value wins over the external fallback.
    assert_eq!(
      block_on_ready(reported_module_side_effects(
        &options(ModuleSideEffects::Boolean(false)),
        &bare
      ))
      .unwrap(),
      HookSideEffects::False,
    );

    let attributes = ImportAttributes::from_entries([("type", "json")]);
    let attributed = ResolvedId {
      id: ModuleId::with_attributes("/src/local.js", &attributes),
      ..Default::default()
    };
    assert_ne!(attributed.id.raw_id(), attributed.id.as_str());
    let raw_only =
      options(ModuleSideEffects::IdSet(FxHashSet::from_iter(["/src/local.js".into()])));
    assert_eq!(
      block_on_ready(reported_module_side_effects(&raw_only, &attributed)).unwrap(),
      HookSideEffects::True,
    );
    let full_only =
      options(ModuleSideEffects::IdSet(FxHashSet::from_iter([attributed.id.as_str().into()])));
    assert_eq!(
      block_on_ready(reported_module_side_effects(&full_only, &attributed)).unwrap(),
      HookSideEffects::False,
    );
  }
}
