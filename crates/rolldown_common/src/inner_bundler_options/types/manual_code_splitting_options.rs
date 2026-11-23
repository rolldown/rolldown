use std::{borrow::Cow, ops::Deref, pin::Pin, sync::Arc};

use rolldown_error::BuildResult;
use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use crate::{
  InvalidateJsSideCache, ModuleInfo, ModuleTag, SharedModuleInfoDashMap,
  SharedModuleSideEffectsOverrides,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};

use super::inline_common_chunks_options::InlineCommonChunksOptions;

/// Schema-only enum for built-in module tags. Used by `schemars` to generate
/// a restricted JSON Schema for the `tags` field. Not used at runtime.
#[cfg(feature = "deserialize_bundler_options")]
#[derive(JsonSchema)]
#[expect(dead_code)]
enum BuiltinModuleTag {
  #[schemars(rename = "$initial")]
  Initial,
  // Phase 2:
  // #[schemars(rename = "$lazy")]
  // Lazy,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ManualCodeSplittingOptions {
  pub min_share_count: Option<u32>,
  pub min_size: Option<f64>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub include_dependencies_recursively: Option<bool>,
  pub groups: Option<Vec<MatchGroup>>,
  /// See internal-docs/inline-common-chunks/design.md. Experimental.
  pub experimental_inline_common_chunks: Option<InlineCommonChunksOptions>,
  #[cfg_attr(feature = "deserialize_bundler_options", serde(skip), schemars(skip))]
  pub internal_invalidate_module_info_cache: Option<InvalidateJsSideCache>,
}

#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct MatchGroup {
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_match_group_name"),
    schemars(with = "String")
  )]
  pub name: MatchGroupName,
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    serde(deserialize_with = "deserialize_test", default),
    schemars(with = "Option<String>")
  )]
  pub test: Option<MatchGroupTest>,
  // pub share_count: Option<u32>,
  pub priority: Option<u32>,
  pub min_size: Option<f64>,
  pub max_size: Option<f64>,
  pub min_share_count: Option<u32>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub entries_aware: Option<bool>,
  /// Only effective when `entriesAware` is set to `true`.
  pub entries_aware_merge_threshold: Option<f64>,
  /// Filter modules by tags. Only modules with all specified tags are captured.
  /// See internal-docs/module-tags/implementation.md
  #[cfg_attr(
    feature = "deserialize_bundler_options",
    schemars(with = "Option<Vec<BuiltinModuleTag>>")
  )]
  pub tags: Option<Vec<ModuleTag>>,
  pub include_dependencies_recursively: Option<bool>,
}

/// Classifies a whole batch of module ids in one call.
///
/// The returned `Vec` must match `module_ids` by index. The caller rejects any other length.
type MatchGroupTestFn = dyn Fn(
    /* module ids */ Vec<String>,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<bool>>> + Send + 'static>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Clone)]
pub enum MatchGroupTest {
  Regex(HybridRegex),
  #[debug("Function")]
  Function(Arc<MatchGroupTestFn>),
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_test<'de, D>(deserializer: D) -> Result<Option<MatchGroupTest>, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = Option::<String>::deserialize(deserializer)?;
  let transformed = deserialized
    .map(|inner| HybridRegex::new(&inner))
    .transpose()
    .map_err(|e| serde::de::Error::custom(format!("failed to deserialize {e} to HybridRegex")))?;
  Ok(transformed.map(MatchGroupTest::Regex))
}

/// Names a whole batch of module ids in one call. The returned `Vec` follows the same rule as
/// [`MatchGroupTestFn`].
type MatchGroupNameFn = dyn Fn(
    /* module ids */ Vec<String>,
    /* chunking context */ &ChunkingContext,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<Option<String>>>> + Send + 'static>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Clone)]
pub enum MatchGroupName {
  Static(String),
  #[debug("Function")]
  Dynamic(Arc<MatchGroupNameFn>),
}

impl MatchGroupName {
  /// Resolves the group name of every id in `module_ids`, in the same order. The result always
  /// has the same length as `module_ids`.
  pub async fn values<'a>(
    &'a self,
    ctx: &ChunkingContext,
    module_ids: Vec<String>,
  ) -> BuildResult<Vec<Option<Cow<'a, str>>>> {
    match self {
      Self::Static(name) => Ok(vec![Some(Cow::Borrowed(name.as_str())); module_ids.len()]),
      Self::Dynamic(func) => {
        let expected = module_ids.len();
        let names = func(module_ids, ctx).await?;
        if names.len() != expected {
          return Err(
            anyhow::anyhow!(
              "a `codeSplitting` group `name` function returned {} names for {expected} modules",
              names.len()
            )
            .into(),
          );
        }
        Ok(names.into_iter().map(|name| name.map(Cow::Owned)).collect())
      }
    }
  }
}

impl Default for MatchGroupName {
  fn default() -> Self {
    Self::Static(String::new())
  }
}

#[cfg(feature = "deserialize_bundler_options")]
fn deserialize_match_group_name<'de, D>(deserializer: D) -> Result<MatchGroupName, D::Error>
where
  D: Deserializer<'de>,
{
  let deserialized = String::deserialize(deserializer)?;
  Ok(MatchGroupName::Static(deserialized))
}

#[derive(Debug, Clone)]
pub struct ChunkingContext(Arc<ChunkingContextImpl>);

impl ChunkingContext {
  pub fn new(
    module_infos: SharedModuleInfoDashMap,
    module_side_effects_overrides: SharedModuleSideEffectsOverrides,
  ) -> Self {
    Self(Arc::new(ChunkingContextImpl { module_infos, module_side_effects_overrides }))
  }
}

impl Deref for ChunkingContext {
  type Target = ChunkingContextImpl;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Debug)]
pub struct ChunkingContextImpl {
  module_infos: SharedModuleInfoDashMap,
  module_side_effects_overrides: SharedModuleSideEffectsOverrides,
}

impl ChunkingContextImpl {
  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<ModuleInfo>> {
    let info = self.module_infos.get(module_id).map(|v| Arc::<ModuleInfo>::clone(v.value()))?;
    Some(self.module_side_effects_overrides.apply_to_info(info))
  }

  /// `get_module_info(module_id)?.side_effects?.shown()`, without copying the info. It does not
  /// run a `treeshake.moduleSideEffects` function, so it is `None` in the one case that needs a
  /// call that has not run: a `null` write from Rust, here or in an output hook, for a module
  /// whose value a hook decided.
  pub fn get_module_side_effects(&self, module_id: &str) -> Option<DeterminedSideEffects> {
    let info = self.module_infos.get(module_id)?;
    self.module_side_effects_overrides.shown(&info)
  }

  /// Like `get_module_side_effects`, but runs `call_option(raw_id, external)` when the value
  /// needs the option function. The binding passes the JS function.
  pub fn get_module_side_effects_or_call(
    &self,
    module_id: &str,
    call_option: impl FnOnce(&str, bool) -> anyhow::Result<Option<bool>>,
  ) -> anyhow::Result<Option<DeterminedSideEffects>> {
    let Some(info) = self.module_infos.get(module_id).map(|v| Arc::clone(v.value())) else {
      return Ok(None);
    };
    self.module_side_effects_overrides.shown_or_call(&info, call_option)
  }

  /// A write to `getModuleInfo(module_id).moduleSideEffects`. `None` is a write of `null`.
  /// The chunking context runs after `buildEnd`, so the write is ignored. Returns `true` when the
  /// caller should warn. See [`ModuleSideEffectsOverrides::write`].
  pub fn set_module_side_effects(
    &self,
    module_id: &str,
    side_effects: Option<HookSideEffects>,
  ) -> bool {
    self.module_side_effects_overrides.write(module_id, side_effects)
  }
}
