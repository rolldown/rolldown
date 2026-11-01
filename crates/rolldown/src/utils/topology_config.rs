//! Declarative topology heuristics, overfetch bounds, and semantic grouping policies for
//! `generate_stage`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CycleMergingPolicy {
  #[default]
  Forbid,
  AllowWrapped,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommonChunkCoalescing {
  #[default]
  ByEntrySet,
  Off,
  Aggressive,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyBoundary {
  /// Default Rolldown behavior (`include_dependencies_recursively: true`).
  #[default]
  All,
  /// Only seed modules (`include_dependencies_recursively: false`).
  None,
  /// App/workspace seed modules only recursively pull app/workspace modules (stopping at
  /// `node_modules`), while vendor seed modules only pull vendor dependencies.
  SameOriginKind,
  /// Seed modules only recursively pull dependencies within the same `package.json` name.
  SamePackage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkSizeBudget {
  pub min_bytes: u32,
  pub target_bytes: Option<u32>,
  pub max_bytes: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupNamingPolicy {
  Static(String),
  ByPackage {
    prefix: String,
    disambiguate_version: bool,
  },
  ByDirectory {
    base_name: String,
    prefix: Option<String>,
    depth: u8,
  },
  ByEcosystem {
    prefix: String,
  },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GroupRuleConfig {
  pub packages: Vec<String>,
  pub paths: Vec<String>,
  pub naming: Option<GroupNamingPolicy>,
  pub boundary: DependencyBoundary,
  pub target_bytes: Option<u32>,
  pub overfetch_budget_bytes: Option<u32>,
  pub is_mf_shared: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TopologyConfig {
  pub chunk_size: Option<ChunkSizeBudget>,
  pub max_initial_requests: Option<u16>,
  pub max_async_requests: Option<u16>,
  pub overfetch_budget_bytes: Option<u32>,
  pub cycle_merging: Option<CycleMergingPolicy>,
  pub common_chunk_coalescing: Option<CommonChunkCoalescing>,
  /// Per-match-group semantic rule, indexed by `match_group_index`.
  pub group_rules: Vec<GroupRuleConfig>,
  /// Per-match-group dependency boundary, indexed by `match_group_index`.
  pub group_boundaries: Vec<DependencyBoundary>,
  /// Per-match-group target size override, indexed by `match_group_index`.
  pub group_target_bytes: Vec<Option<u32>>,
  /// Per-match-group overfetch budget override, indexed by `match_group_index`.
  pub group_overfetch_budget_bytes: Vec<Option<u32>>,
  /// Package names (`package.json#name`) isolated as Module Federation shared singletons.
  /// Non-shared groups (`DependencyBoundary != All` or lacking an explicit `mf-shared-*` allow)
  /// never recursively pull modules belonging to these packages.
  pub mf_shared_packages: Vec<String>,
  /// Per-match-group flag indicating whether the group is itself a synthesized `mf-shared-*` group
  /// allowed to capture `mf_shared_packages`.
  pub group_is_mf_shared: Vec<bool>,
  /// Whether UserDefinedEntry chunks represent Module Federation remote container entries that must
  /// not absorb non-eager exposed modules or shared chunks.
  pub preserve_container_entries: bool,
}

pub const TOPOLOGY_KEY_HEADER: &str = "\0__native_topology_config_json:";

pub fn attach(options: &mut rolldown_common::BundlerOptions, config: TopologyConfig) {
  let encoded = serde_json::to_string(&config).expect("TopologyConfig is always serializable");
  let tag = format!("{TOPOLOGY_KEY_HEADER}{encoded}");
  let labels = options.drop_labels.get_or_insert_default();
  labels.retain(|label| !label.starts_with(TOPOLOGY_KEY_HEADER));
  labels.push(tag);
}

pub fn get_from_raw(options: &rolldown_common::BundlerOptions) -> Option<Arc<TopologyConfig>> {
  for label in options.drop_labels.as_deref().unwrap_or_default() {
    if let Some(payload) = label.strip_prefix(TOPOLOGY_KEY_HEADER)
      && let Ok(parsed) = serde_json::from_str::<TopologyConfig>(payload)
    {
      return Some(Arc::new(parsed));
    }
  }
  None
}

pub fn get(options: &rolldown_common::NormalizedBundlerOptions) -> Option<Arc<TopologyConfig>> {
  for label in &options.drop_labels {
    if let Some(payload) = label.strip_prefix(TOPOLOGY_KEY_HEADER)
      && let Ok(parsed) = serde_json::from_str::<TopologyConfig>(payload)
    {
      return Some(Arc::new(parsed));
    }
  }
  None
}
