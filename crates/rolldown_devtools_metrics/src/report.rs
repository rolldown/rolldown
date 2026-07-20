//! The canonical machine-readable report model. `metrics.json` is the serialization of
//! [`Report`], and every markdown file is rendered from this same model, so the JSON and the
//! human views can never drift.
//!
//! The flat [`Report::metrics`] map (`metric id -> number`) is the interchange view: deltas,
//! `history.jsonl` lines, and any future remote ingestion all key off these ids. Ids are stable
//! and append-only, with the unit suffixed (`_ms`, `_bytes`, `_count`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{ChunkAgg, MetricsAggregator};

pub(crate) const SCHEMA_VERSION: u32 = 1;

/// One package's `(name, [(version, rendered bytes)])` duplicate-version rows.
type PackageVersionRows = (String, Vec<(String, u64)>);

/// A single numeric metric value. Counts and byte sizes stay integers; durations are
/// fractional milliseconds. Serialized as a plain JSON number either way.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(untagged)]
pub(crate) enum MetricValue {
  Int(u64),
  Float(f64),
}

/// Hand-written instead of `#[serde(untagged)]`: untagged numeric enums fail to deserialize
/// when `serde_json` is built with `arbitrary_precision` (numbers buffer as an internal map,
/// matching neither variant), and feature unification turns that on for any build that
/// includes `rolldown_common`. Going through `serde_json::Number` works under both states.
impl<'de> Deserialize<'de> for MetricValue {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let number = serde_json::Number::deserialize(deserializer)?;
    if let Some(int) = number.as_u64() {
      return Ok(Self::Int(int));
    }
    number
      .as_f64()
      .map(Self::Float)
      .ok_or_else(|| serde::de::Error::custom("metric value is not a finite number"))
  }
}

impl MetricValue {
  pub(crate) fn as_f64(self) -> f64 {
    match self {
      Self::Int(v) => v as f64,
      Self::Float(v) => v,
    }
  }
}

fn round3(v: f64) -> f64 {
  (v * 1000.0).round() / 1000.0
}

fn round1(v: f64) -> f64 {
  (v * 10.0).round() / 10.0
}

fn ms_value(micros: u64) -> f64 {
  round3(micros as f64 / 1000.0)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Report {
  pub schema_version: u32,
  /// Wall-clock unix ms; stamped at render time (left out of unit-built reports so test
  /// snapshots stay stable).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub generated_at_ms: Option<u64>,
  pub session: SessionSection,
  /// Flat `metric id -> value` map. See the module docs; this is the view deltas and
  /// `history.jsonl` are computed from.
  pub metrics: BTreeMap<String, MetricValue>,
  pub entries: Vec<EntrySection>,
  pub chunks: ChunksSection,
  pub assets: Vec<AssetRow>,
  pub modules: ModulesSection,
  /// Dominator-tree retained-size analysis over the static module graph. Present when a
  /// module graph (and per-module rendered sizes) reached this build's event stream.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub graph: Option<GraphSection>,
  pub packages: PackagesSection,
  pub plugins: Vec<PluginSection>,
  pub transform_hotspots: Vec<TransformHotspot>,
  /// Change vs the immediately previous build (`.state.json`).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub delta: Option<DeltaSection>,
  /// Change vs a pinned baseline: present when the metrics dir contains `baseline.json`
  /// (state-shaped; pin by copying `.state.json`). Stays fixed across experiments, so N
  /// config attempts all compare against the same reference build.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub baseline_delta: Option<DeltaSection>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSection {
  pub cwd: Option<String>,
  pub platform: Option<String>,
  pub format: Option<String>,
  pub input_count: usize,
  pub plugin_count: usize,
  /// 1-based index of this build within the session (watch mode reuses a session).
  pub build_index: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntrySection {
  /// Stabilized entry module path — the stable join key for deltas across builds.
  pub entry: String,
  pub chunk: String,
  pub chunk_bytes: u64,
  /// Entry chunk bytes + all transitively static-imported chunk bytes (dynamic imports are
  /// lazy, so excluded). The code-splitting KPI.
  pub initial_load_bytes: u64,
  pub static_imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunksSection {
  /// Non-empty output chunks (placeholder chunks with 0 bytes and 0 modules excluded).
  pub count: usize,
  pub reasons: BTreeMap<String, usize>,
  pub largest: Vec<ChunkRow>,
  pub duplicated_module_count: usize,
  pub duplicated_modules: Vec<DuplicatedModule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkRow {
  pub file: String,
  pub bytes: u64,
  pub is_entry: bool,
  pub reason: String,
  pub module_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicatedModule {
  pub module: String,
  pub chunks: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssetRow {
  pub file: String,
  pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModulesSection {
  pub count: usize,
  pub external_count: usize,
  pub import_kinds: BTreeMap<String, usize>,
  pub most_imported: Vec<MostImported>,
  pub entry_point_count: usize,
  pub shared_across_entries_count: usize,
  pub shared_across_entries: Vec<SharedModule>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MostImported {
  pub module: String,
  pub importers: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedModule {
  pub module: String,
  pub entries: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GraphSection {
  /// User-defined entry modules — the roots of the dominator tree.
  pub entry_modules: Vec<String>,
  /// Modules reachable from the entries over static edges (= part of some initial load).
  pub static_module_count: usize,
  pub static_bytes: u64,
  /// Modules reachable only across a `dynamic-import` edge (already lazy).
  pub dynamic_only_module_count: usize,
  /// Top non-entry modules by retained size: deferring the import edge that pulls the module
  /// in would remove `retained_bytes` (the whole dominator subtree) from the initial load.
  pub retained_top: Vec<RetainedRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RetainedRow {
  pub module: String,
  /// The module's own rendered bytes.
  pub bytes: u64,
  /// Own bytes + everything only reachable through it (its dominator subtree).
  pub retained_bytes: u64,
  pub retained_module_count: usize,
  /// The immediate dominator — the module whose import chain is the single way in. Absent
  /// when the module hangs directly off the entries (an entry itself, or a join point
  /// shared by several entries).
  #[serde(skip_serializing_if = "Option::is_none")]
  pub via: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackagesSection {
  pub count: usize,
  pub direct_count: usize,
  pub transitive_count: usize,
  pub largest: Vec<PackageRow>,
  pub duplicate_version_count: usize,
  pub duplicate_versions: Vec<DuplicateVersions>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PackageRow {
  pub name: String,
  pub version: Option<String>,
  pub bytes: u64,
  pub module_count: usize,
  pub dependency_type: String,
  pub used: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DuplicateVersions {
  pub name: String,
  pub versions: Vec<VersionBytes>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VersionBytes {
  pub version: String,
  pub bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PluginSection {
  pub name: String,
  pub calls: usize,
  pub ms: f64,
  /// Calls that did no work, summed across hooks — see [`HookRow::noop_calls`].
  pub noop_calls: usize,
  pub hooks: BTreeMap<String, HookRow>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HookRow {
  pub calls: usize,
  pub ms: f64,
  /// Calls that did no work: `resolveId`/`load`/`renderChunk` returned nothing, `transform`
  /// returned nothing or the code unchanged. High no-op share on a hot hook ⇒ the plugin
  /// should declare a hook filter.
  pub noop_calls: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TransformHotspot {
  pub module: String,
  pub calls: usize,
  pub ms: f64,
}

// --- delta ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeltaSection {
  /// Per-metric change, for every metric id present in both builds.
  pub metrics: BTreeMap<String, MetricDelta>,
  /// Per-entry initial-load change, joined by entry module.
  pub entries: Vec<EntryDelta>,
  pub added_entries: Vec<String>,
  pub removed_entries: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricDelta {
  pub prev: MetricValue,
  pub curr: MetricValue,
  pub delta: f64,
  /// Percent change; absent when the previous value was 0.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pct: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryDelta {
  pub entry: String,
  pub initial_load_bytes: MetricDelta,
}

pub(crate) fn metric_delta(prev: MetricValue, curr: MetricValue) -> MetricDelta {
  let (p, c) = (prev.as_f64(), curr.as_f64());
  MetricDelta {
    prev,
    curr,
    delta: round3(c - p),
    pct: (p != 0.0).then(|| round1((c - p) / p * 100.0)),
  }
}

// --- persisted state (previous build snapshot, used only to compute the next delta) ---

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MetricsState {
  pub schema_version: u32,
  pub metrics: BTreeMap<String, MetricValue>,
  #[serde(default)]
  pub entries: Vec<EntryState>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EntryState {
  pub entry: String,
  pub chunk_bytes: u64,
  pub initial_load_bytes: u64,
}

// --- history (one flat line per build, appended to history.jsonl) ---

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryLine {
  pub schema_version: u32,
  /// Wall-clock unix ms at append time.
  pub ts_ms: u64,
  /// 1-based build index within the session.
  pub build: usize,
  pub metrics: BTreeMap<String, MetricValue>,
  pub entries: Vec<EntryState>,
}

impl Report {
  pub(crate) fn to_state(&self) -> MetricsState {
    MetricsState {
      schema_version: SCHEMA_VERSION,
      metrics: self.metrics.clone(),
      entries: self.entries.iter().map(EntrySection::to_state).collect(),
    }
  }
}

impl EntrySection {
  fn to_state(&self) -> EntryState {
    EntryState {
      entry: self.entry.clone(),
      chunk_bytes: self.chunk_bytes,
      initial_load_bytes: self.initial_load_bytes,
    }
  }
}

fn compute_delta(
  prev: &MetricsState,
  metrics: &BTreeMap<String, MetricValue>,
  entries: &[EntrySection],
) -> DeltaSection {
  let mut metric_rows = BTreeMap::new();
  for (id, curr) in metrics {
    if let Some(prev_value) = prev.metrics.get(id) {
      metric_rows.insert(id.clone(), metric_delta(*prev_value, *curr));
    }
  }
  let prev_entries: BTreeMap<&str, &EntryState> =
    prev.entries.iter().map(|e| (e.entry.as_str(), e)).collect();
  let mut entry_rows = Vec::new();
  let mut added = Vec::new();
  for entry in entries {
    match prev_entries.get(entry.entry.as_str()) {
      Some(prev_entry) => entry_rows.push(EntryDelta {
        entry: entry.entry.clone(),
        initial_load_bytes: metric_delta(
          MetricValue::Int(prev_entry.initial_load_bytes),
          MetricValue::Int(entry.initial_load_bytes),
        ),
      }),
      None => added.push(entry.entry.clone()),
    }
  }
  let curr_names: std::collections::BTreeSet<&str> =
    entries.iter().map(|e| e.entry.as_str()).collect();
  let removed = prev
    .entries
    .iter()
    .filter(|e| !curr_names.contains(e.entry.as_str()))
    .map(|e| e.entry.clone())
    .collect();
  DeltaSection {
    metrics: metric_rows,
    entries: entry_rows,
    added_entries: added,
    removed_entries: removed,
  }
}

// --- building the report from the aggregates ---

impl MetricsAggregator {
  /// Build the canonical report. `prev` (the persisted previous-build state) enables the
  /// `delta` section; `baseline` (a user-pinned state) enables `baseline_delta`.
  /// `generated_at_ms` is left `None`; `render()` stamps it.
  pub(crate) fn build_report(
    &self,
    prev: Option<&MetricsState>,
    baseline: Option<&MetricsState>,
  ) -> Report {
    let entries = self.entry_sections();
    let metrics = self.flat_metrics(&entries);
    let delta = prev.map(|p| compute_delta(p, &metrics, &entries));
    let baseline_delta = baseline.map(|b| compute_delta(b, &metrics, &entries));
    Report {
      schema_version: SCHEMA_VERSION,
      generated_at_ms: None,
      session: SessionSection {
        cwd: self.cwd.clone(),
        platform: self.platform.clone(),
        format: self.format.clone(),
        input_count: self.input_count,
        plugin_count: self.plugin_count,
        build_index: self.build_index,
      },
      metrics,
      entries,
      chunks: self.chunks_section(),
      assets: self
        .assets
        .iter()
        .map(|a| AssetRow { file: a.filename.clone(), bytes: a.size })
        .collect(),
      modules: self.modules_section(),
      graph: self.graph_analysis().map(|analysis| self.graph_section(&analysis)),
      packages: self.packages_section(),
      plugins: self.plugins_section(),
      transform_hotspots: self.transform_hotspots_section(),
      delta,
      baseline_delta,
    }
  }

  /// The current build's history line, or `None` when no build completed.
  pub(crate) fn history_line(&self, ts_ms: u64) -> Option<HistoryLine> {
    self.build_end?;
    let entries = self.entry_sections();
    let metrics = self.flat_metrics(&entries);
    Some(HistoryLine {
      schema_version: SCHEMA_VERSION,
      ts_ms,
      build: self.build_index,
      metrics,
      entries: entries.iter().map(EntrySection::to_state).collect(),
    })
  }

  fn duration_ms(&self, start: Option<std::time::Instant>, end: Option<std::time::Instant>) -> f64 {
    match (start, end) {
      (Some(start), Some(end)) if end >= start => {
        round3(end.duration_since(start).as_secs_f64() * 1000.0)
      }
      _ => 0.0,
    }
  }

  fn flat_metrics(&self, entries: &[EntrySection]) -> BTreeMap<String, MetricValue> {
    use MetricValue::{Float, Int};
    let (dup_module_total, _) = self.duplicated_modules();
    let (_, shared_total, _) = self.reach_from_entries();
    let (dup_version_total, _) = self.duplicate_package_versions();
    let hook_calls: usize = self.hook_calls.values().map(|s| s.count).sum();
    let hook_micros: u64 = self.hook_calls.values().map(|s| s.micros).sum();
    let hook_noops: usize = self.hook_calls.values().map(|s| s.noop_calls).sum();
    let initial_load_max = entries.iter().map(|e| e.initial_load_bytes).max().unwrap_or(0);
    let as_u64 = |v: usize| Int(v as u64);
    let mut metrics = BTreeMap::new();
    let mut put = |id: &str, value: MetricValue| {
      metrics.insert(id.to_string(), value);
    };
    put("build.total_ms", Float(self.duration_ms(self.build_start, self.build_end)));
    put("build.scan_ms", Float(self.duration_ms(self.build_start, self.scan_end)));
    put("build.generate_ms", Float(self.duration_ms(self.scan_end, self.build_end)));
    put("output.total_bytes", Int(self.total_bytes));
    put("output.js_bytes", Int(self.js_bytes));
    put("output.css_bytes", Int(self.css_bytes));
    put("output.other_bytes", Int(self.other_bytes));
    put("output.chunk_count", as_u64(self.chunk_count()));
    put("output.asset_count", as_u64(self.asset_count));
    put("output.entry_count", as_u64(entries.len()));
    put("output.max_initial_load_bytes", Int(initial_load_max));
    put("modules.count", as_u64(self.module_count));
    put("modules.external_count", as_u64(self.external_count));
    put("modules.shared_across_entries_count", as_u64(shared_total));
    put("chunks.duplicated_module_count", as_u64(dup_module_total));
    put("packages.count", as_u64(self.package_direct + self.package_transitive));
    put("packages.direct_count", as_u64(self.package_direct));
    put("packages.transitive_count", as_u64(self.package_transitive));
    put("packages.duplicate_version_count", as_u64(dup_version_total));
    put("plugins.hook_call_count", as_u64(hook_calls));
    put("plugins.hook_noop_call_count", as_u64(hook_noops));
    put("plugins.hook_time_ms", Float(ms_value(hook_micros)));
    metrics
  }

  /// Entry chunks as report sections, sorted by entry module for a diff-stable order.
  fn entry_sections(&self) -> Vec<EntrySection> {
    let mut entries: Vec<EntrySection> = self
      .chunks
      .iter()
      .filter(|(_, chunk)| chunk.is_entry)
      .map(|(id, chunk)| {
        let entry = chunk
          .entry_module
          .as_ref()
          .map_or_else(|| self.chunk_label(*id), |module| self.stabilize(module));
        let labels = |ids: &[u32]| {
          let mut labels: Vec<String> = ids.iter().map(|i| self.chunk_label(*i)).collect();
          labels.sort();
          labels.dedup();
          labels
        };
        EntrySection {
          entry,
          chunk: self.chunk_label(*id),
          chunk_bytes: self.chunk_sizes.get(id).copied().unwrap_or(0),
          initial_load_bytes: self.initial_load_bytes(*id),
          static_imports: labels(&chunk.static_imports),
          dynamic_imports: labels(&chunk.dynamic_imports),
        }
      })
      .collect();
    entries.sort_by(|a, b| a.entry.cmp(&b.entry).then_with(|| a.chunk.cmp(&b.chunk)));
    entries.truncate(self.config.top_n);
    entries
  }

  fn chunks_section(&self) -> ChunksSection {
    let chunks = self.chunks_with_size();
    let mut reasons: BTreeMap<String, usize> = BTreeMap::new();
    for (_, chunk, _) in &chunks {
      *reasons.entry(chunk.reason.clone()).or_default() += 1;
    }
    let largest = chunks
      .iter()
      .take(self.config.top_n)
      .map(|(id, chunk, bytes)| ChunkRow {
        file: self.chunk_label(*id),
        bytes: *bytes,
        is_entry: chunk.is_entry,
        reason: chunk.reason.clone(),
        module_count: chunk.module_count,
      })
      .collect();
    let (duplicated_module_count, duplicated_modules) = self.duplicated_modules();
    ChunksSection {
      count: chunks.len(),
      reasons,
      largest,
      duplicated_module_count,
      duplicated_modules: duplicated_modules
        .into_iter()
        .map(|(module, chunks)| DuplicatedModule { module, chunks })
        .collect(),
    }
  }

  fn modules_section(&self) -> ModulesSection {
    let (entry_point_count, shared_total, shared) = self.reach_from_entries();
    ModulesSection {
      count: self.module_count,
      external_count: self.external_count,
      import_kinds: self.import_kind_hist.iter().map(|(k, v)| (k.clone(), *v)).collect(),
      most_imported: self
        .most_imported
        .iter()
        .map(|(id, importers)| MostImported { module: self.stabilize(id), importers: *importers })
        .collect(),
      entry_point_count,
      shared_across_entries_count: shared_total,
      shared_across_entries: shared
        .into_iter()
        .map(|(module, entries)| SharedModule { module, entries })
        .collect(),
    }
  }

  fn packages_section(&self) -> PackagesSection {
    let (duplicate_version_count, duplicate_versions) = self.duplicate_package_versions();
    PackagesSection {
      count: self.package_direct + self.package_transitive,
      direct_count: self.package_direct,
      transitive_count: self.package_transitive,
      largest: self
        .packages
        .iter()
        .map(|package| PackageRow {
          name: package.name.clone(),
          version: package.version.clone(),
          bytes: package.size,
          module_count: package.module_count,
          dependency_type: package.dependency_type.clone(),
          used: package.is_used,
        })
        .collect(),
      duplicate_version_count,
      duplicate_versions: duplicate_versions
        .into_iter()
        .map(|(name, versions)| DuplicateVersions {
          name,
          versions: versions
            .into_iter()
            .map(|(version, bytes)| VersionBytes { version, bytes })
            .collect(),
        })
        .collect(),
    }
  }

  fn plugins_section(&self) -> Vec<PluginSection> {
    let mut by_plugin: BTreeMap<&str, PluginSection> = BTreeMap::new();
    for ((plugin, hook), stat) in &self.hook_calls {
      let entry = by_plugin.entry(plugin.as_str()).or_insert_with(|| PluginSection {
        name: plugin.clone(),
        calls: 0,
        ms: 0.0,
        noop_calls: 0,
        hooks: BTreeMap::new(),
      });
      entry.calls += stat.count;
      entry.ms += stat.micros as f64 / 1000.0;
      entry.noop_calls += stat.noop_calls;
      entry.hooks.insert(
        (*hook).to_string(),
        HookRow { calls: stat.count, ms: ms_value(stat.micros), noop_calls: stat.noop_calls },
      );
    }
    let mut plugins: Vec<PluginSection> = by_plugin.into_values().collect();
    for plugin in &mut plugins {
      plugin.ms = round3(plugin.ms);
    }
    plugins.sort_by(|a, b| {
      b.ms.partial_cmp(&a.ms).unwrap_or(std::cmp::Ordering::Equal).then_with(|| a.name.cmp(&b.name))
    });
    plugins.truncate(self.config.top_n);
    plugins
  }

  fn transform_hotspots_section(&self) -> Vec<TransformHotspot> {
    let mut modules: Vec<TransformHotspot> = self
      .module_transform
      .iter()
      .map(|(id, stat)| TransformHotspot {
        module: self.stabilize(id),
        calls: stat.count,
        ms: ms_value(stat.micros),
      })
      .collect();
    modules.sort_by(|a, b| {
      b.ms
        .partial_cmp(&a.ms)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.module.cmp(&b.module))
    });
    modules.truncate(self.config.top_n);
    modules
  }

  /// Real (non-empty) output chunk count. Pure placeholders — 0 bytes AND 0 modules, e.g. the
  /// runtime stub or async-entry stubs whose code was merged elsewhere — are excluded.
  fn chunk_count(&self) -> usize {
    self
      .chunks
      .iter()
      .filter(|(id, c)| c.module_count > 0 || self.chunk_sizes.get(id).copied().unwrap_or(0) > 0)
      .count()
  }

  /// Non-empty chunks as `(id, agg, bytes)`, sorted by size desc with a deterministic tie-break.
  fn chunks_with_size(&self) -> Vec<(u32, &ChunkAgg, u64)> {
    let mut chunks: Vec<(u32, &ChunkAgg, u64)> = self
      .chunks
      .iter()
      .map(|(id, c)| (*id, c, self.chunk_sizes.get(id).copied().unwrap_or(0)))
      .filter(|(_, c, size)| c.module_count > 0 || *size > 0)
      .collect();
    chunks.sort_by(|a, b| {
      b.2.cmp(&a.2).then_with(|| self.chunk_label(a.0).cmp(&self.chunk_label(b.0)))
    });
    chunks
  }

  /// Modules bundled into >1 chunk (they ship multiple times). Returns (total, top-N rows of
  /// `(module, distinct chunk labels)`), most-duplicated first.
  fn duplicated_modules(&self) -> (usize, Vec<(String, Vec<String>)>) {
    let mut all: Vec<(String, Vec<String>)> = self
      .module_chunks
      .iter()
      .filter_map(|(id, chunks)| {
        let mut labels: Vec<String> = chunks.iter().map(|c| self.chunk_label(*c)).collect();
        labels.sort();
        labels.dedup();
        (labels.len() > 1).then(|| (self.stabilize(id), labels))
      })
      .collect();
    all.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    let total = all.len();
    all.truncate(self.config.top_n);
    (total, all)
  }

  /// The user-defined entry modules (dominator roots); async entries excluded, with a
  /// fallback to any entry chunk for builds that only have async entries.
  fn user_entry_modules(&self) -> Vec<&str> {
    let mut entries: Vec<&str> = self
      .chunks
      .values()
      .filter(|c| c.is_user_entry)
      .filter_map(|c| c.entry_module.as_deref())
      .collect();
    if entries.is_empty() {
      entries = self
        .chunks
        .values()
        .filter(|c| c.is_entry)
        .filter_map(|c| c.entry_module.as_deref())
        .collect();
    }
    entries.sort_unstable();
    entries.dedup();
    entries
  }

  /// Dominator-tree retained-size analysis over the static module graph. `None` when no
  /// module graph reached this build (metrics without devtools module events).
  pub(crate) fn graph_analysis(&self) -> Option<crate::graph::GraphAnalysis> {
    if self.module_imports.is_empty() {
      return None;
    }
    let entries = self.user_entry_modules();
    crate::graph::analyze(&self.module_imports, &self.module_bytes, &entries)
  }

  pub(crate) fn graph_section(&self, analysis: &crate::graph::GraphAnalysis) -> GraphSection {
    let entry_set: std::collections::BTreeSet<&str> =
      analysis.entry_modules.iter().map(String::as_str).collect();
    let mut rows: Vec<&crate::graph::GraphNode> = analysis
      .nodes
      .iter()
      .filter(|node| {
        node.static_reachable && node.retained_bytes > 0 && !entry_set.contains(node.id.as_str())
      })
      .collect();
    rows.sort_by(|a, b| b.retained_bytes.cmp(&a.retained_bytes).then_with(|| a.id.cmp(&b.id)));
    rows.truncate(self.config.top_n);
    GraphSection {
      entry_modules: analysis.entry_modules.iter().map(|id| self.stabilize(id)).collect(),
      static_module_count: analysis.static_module_count,
      static_bytes: analysis.static_bytes,
      dynamic_only_module_count: analysis.dynamic_only_count,
      retained_top: rows
        .into_iter()
        .map(|node| RetainedRow {
          module: self.stabilize(&node.id),
          bytes: node.bytes,
          retained_bytes: node.retained_bytes,
          retained_module_count: node.retained_count,
          via: node.idom.map(|idx| self.stabilize(&analysis.nodes[idx].id)),
        })
        .collect(),
    }
  }

  /// Modules reachable from >1 entry point (the real shared-chunk signal, vs raw import
  /// fan-in). Returns (entry count, total shared, top-N rows of `(module, #entries)`).
  fn reach_from_entries(&self) -> (usize, usize, Vec<(String, usize)>) {
    let mut entries: Vec<&str> = self
      .chunks
      .values()
      .filter(|c| c.is_entry)
      .filter_map(|c| c.entry_module.as_deref())
      .collect();
    entries.sort_unstable();
    entries.dedup();
    let mut count: rustc_hash::FxHashMap<&str, usize> = rustc_hash::FxHashMap::default();
    for entry in &entries {
      let mut seen: rustc_hash::FxHashSet<&str> = rustc_hash::FxHashSet::default();
      let mut stack = vec![*entry];
      while let Some(m) = stack.pop() {
        if !seen.insert(m) {
          continue;
        }
        if let Some(deps) = self.module_imports.get(m) {
          for (dep, _) in deps {
            stack.push(dep.as_str());
          }
        }
      }
      for m in seen {
        *count.entry(m).or_default() += 1;
      }
    }
    let mut rows: Vec<(String, usize)> =
      count.into_iter().filter(|(_, n)| *n >= 2).map(|(m, n)| (self.stabilize(m), n)).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let total = rows.len();
    rows.truncate(self.config.top_n);
    (entries.len(), total, rows)
  }

  /// Bytes loaded on first paint for an entry chunk = its own size + all transitively
  /// STATIC-imported chunks (dynamic imports are lazy, so excluded).
  fn initial_load_bytes(&self, entry_chunk: u32) -> u64 {
    let mut seen: rustc_hash::FxHashSet<u32> = rustc_hash::FxHashSet::default();
    let mut stack = vec![entry_chunk];
    let mut total = 0u64;
    while let Some(cid) = stack.pop() {
      if !seen.insert(cid) {
        continue;
      }
      total += self.chunk_sizes.get(&cid).copied().unwrap_or(0);
      if let Some(chunk) = self.chunks.get(&cid) {
        for &s in &chunk.static_imports {
          stack.push(s);
        }
      }
    }
    total
  }

  /// Package names shipped at >1 distinct version. Returns (total, top-N rows of
  /// `(name, [(version, bytes)])`).
  fn duplicate_package_versions(&self) -> (usize, Vec<PackageVersionRows>) {
    let mut dups: Vec<PackageVersionRows> = self
      .package_versions
      .iter()
      .filter_map(|(name, versions)| {
        let mut distinct: Vec<(String, u64)> = Vec::new();
        for (v, s) in versions {
          if !distinct.iter().any(|(dv, _)| dv == v) {
            distinct.push((v.clone(), *s));
          }
        }
        distinct.sort();
        (distinct.len() > 1).then(|| (name.clone(), distinct))
      })
      .collect();
    dups.sort_by(|a, b| a.0.cmp(&b.0));
    let total = dups.len();
    dups.truncate(self.config.top_n);
    (total, dups)
  }

  pub(crate) fn stabilize(&self, id: &str) -> String {
    let normalized = id.replace('\\', "/");
    // node_modules paths -> package-relative (e.g. `picomatch/lib/utils.js`). Slicing after the
    // LAST `/node_modules/` also strips pnpm's `.pnpm/<pkg>@<ver>/node_modules/` nesting, and is
    // independent of where node_modules lives (project, store, symlink, …).
    if let Some(pos) = normalized.rfind("/node_modules/") {
      return normalized[pos + "/node_modules/".len()..].to_string();
    }
    if let Some(rest) = normalized.strip_prefix("node_modules/") {
      return rest.to_string();
    }
    // Project-relative (strip the session cwd).
    if let Some(cwd) = &self.cwd {
      let cwd = cwd.replace('\\', "/");
      if let Some(rest) = normalized.strip_prefix(&cwd) {
        let rest = rest.trim_start_matches('/');
        if !rest.is_empty() {
          return rest.to_string();
        }
      }
    }
    // Virtual modules (`\0`-prefixed).
    if let Some(stripped) = normalized.strip_prefix('\0') {
      return format!("\\0{stripped}");
    }
    normalized
  }

  /// Human-friendly chunk label: prefer the emitted asset filename, then the chunk's own name.
  pub(crate) fn chunk_label(&self, id: u32) -> String {
    self
      .chunk_filenames
      .get(&id)
      .cloned()
      .or_else(|| self.chunks.get(&id).map(|chunk| chunk.name.clone()))
      .unwrap_or_else(|| format!("chunk-{id}"))
  }
}
