use std::{borrow::Cow, ops::Deref, pin::Pin, sync::Arc};

use rolldown_error::SingleBuildResult;
use rolldown_utils::js_regex::HybridRegex;
#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::{Deserialize, Deserializer};

use crate::{ModuleInfo, SharedModuleInfoDashMap};

#[derive(Default, Debug, Clone)]
#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct AdvancedChunksOptions {
  pub min_share_count: Option<u32>,
  pub min_size: Option<f64>,
  pub max_size: Option<f64>,
  pub min_module_size: Option<f64>,
  pub max_module_size: Option<f64>,
  pub include_dependencies_recursively: Option<bool>,
  pub groups: Option<Vec<MatchGroup>>,
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
}

type MatchGroupTestFn = dyn Fn(&str) -> Pin<Box<dyn Future<Output = SingleBuildResult<Option<bool>>> + Send + 'static>>
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
    .map_err(|e| serde::de::Error::custom(format!("failed to deserialize {e:?} to HybridRegex")))?;
  Ok(transformed.map(MatchGroupTest::Regex))
}

type MatchGroupNameFn = dyn Fn(
    /* module id */ &str,
    /* chunking context */ &ChunkingContext,
  ) -> Pin<Box<dyn Future<Output = SingleBuildResult<Option<String>>> + Send + 'static>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Clone)]
pub enum MatchGroupName {
  Static(String),
  #[debug("Function")]
  Dynamic(Arc<MatchGroupNameFn>),
}

impl MatchGroupName {
  pub async fn value<'a>(
    &'a self,
    ctx: &ChunkingContext,
    module_id: &str,
  ) -> SingleBuildResult<Option<Cow<'a, str>>> {
    match self {
      Self::Static(name) => Ok(Some(Cow::Borrowed(name))),
      Self::Dynamic(func) => {
        let name = func(module_id, ctx).await?;
        Ok(name.map(Cow::Owned))
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
  pub fn new(module_infos: SharedModuleInfoDashMap) -> Self {
    Self(Arc::new(ChunkingContextImpl { module_infos }))
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
}

impl ChunkingContextImpl {
  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<ModuleInfo>> {
    self.module_infos.get(module_id).map(|v| Arc::<ModuleInfo>::clone(v.value()))
  }
}
