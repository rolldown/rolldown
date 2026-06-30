//! In-memory aggregation of Rolldown's devtools event stream into a small, agent-readable
//! metrics report.
//!
//! This is the "option C" sink for build metrics: instead of writing the multi-GB JSON-lines
//! devtools log, the writer thread folds the exact same `trace_action!` events (module graph,
//! chunk graph, package graph, assets, per-hook calls) into bounded top-N aggregates here, then
//! renders a progressive markdown directory (`entry.md` + on-demand detail files) at session
//! close. It never retains the large `content` payloads carried by load/transform/renderChunk
//! events — only counts, sizes, graph structure, and (approximate) timing.

mod render;

use std::time::Instant;

use rustc_hash::FxHashMap;
use serde::Deserialize;

/// Configuration for a metrics session, supplied when the session is opened.
#[derive(Debug, Clone)]
pub struct MetricsConfig {
  /// Output directory for the markdown report, relative to cwd (or absolute).
  pub dir: String,
  /// Upper bound for every "top-N" list so output stays small regardless of app size.
  pub top_n: usize,
  /// Whether to read/write `.state.json` and emit a build-over-build delta.
  pub delta: bool,
}

impl Default for MetricsConfig {
  fn default() -> Self {
    Self { dir: "node_modules/.rolldown/metrics".to_string(), top_n: 20, delta: true }
  }
}

impl MetricsConfig {
  /// Build a config, falling back to defaults for any unset option.
  #[must_use]
  pub fn new(dir: Option<String>, top_n: Option<usize>, delta: Option<bool>) -> Self {
    let defaults = Self::default();
    Self {
      dir: dir.unwrap_or(defaults.dir),
      top_n: top_n.unwrap_or(defaults.top_n),
      delta: delta.unwrap_or(defaults.delta),
    }
  }
}

#[derive(Default)]
pub(crate) struct ChunkAgg {
  pub(crate) name: String,
  pub(crate) reason: String,
  pub(crate) is_entry: bool,
  pub(crate) entry_module: Option<String>,
  pub(crate) module_count: usize,
  pub(crate) static_imports: Vec<u32>,
  pub(crate) dynamic_imports: Vec<u32>,
}

pub(crate) struct PackageAgg {
  pub(crate) name: String,
  pub(crate) version: Option<String>,
  pub(crate) dependency_type: String,
  pub(crate) size: u64,
  pub(crate) module_count: usize,
  pub(crate) is_used: bool,
}

pub(crate) struct AssetAgg {
  pub(crate) filename: String,
  pub(crate) size: u64,
}

#[derive(Default)]
pub(crate) struct HookStat {
  pub(crate) count: usize,
  pub(crate) micros: u64,
}

struct PendingCall {
  plugin: String,
  hook: &'static str,
  start: Instant,
  module_id: Option<String>,
}

/// Accumulates one session's devtools events into bounded aggregates.
#[derive(Default)]
pub struct MetricsAggregator {
  config: MetricsConfig,

  // session meta (set once)
  cwd: Option<String>,
  platform: Option<String>,
  format: Option<String>,
  input_count: usize,
  plugin_count: usize,

  // build timing (writer-side, approximate)
  build_start: Option<Instant>,
  build_end: Option<Instant>,

  // modules
  module_count: usize,
  external_count: usize,
  import_kind_hist: FxHashMap<String, usize>,
  most_imported: Vec<(String, usize)>,

  // chunks
  chunk_reason_hist: FxHashMap<String, usize>,
  chunks: FxHashMap<u32, ChunkAgg>,

  // packages
  package_direct: usize,
  package_transitive: usize,
  packages: Vec<PackageAgg>,

  // assets / sizes
  asset_count: usize,
  total_bytes: u64,
  js_bytes: u64,
  css_bytes: u64,
  other_bytes: u64,
  chunk_sizes: FxHashMap<u32, u64>,
  chunk_filenames: FxHashMap<u32, String>,
  assets: Vec<AssetAgg>,

  // hooks
  hook_calls: FxHashMap<(String, &'static str), HookStat>,
  module_transform: FxHashMap<String, HookStat>,
  pending: FxHashMap<String, PendingCall>,
}

impl MetricsAggregator {
  #[must_use]
  pub fn new(config: MetricsConfig) -> Self {
    Self { config, ..Default::default() }
  }

  /// Fold one resolved devtools action (a JSON object that already has `session_id`/`build_id`
  /// injected) into the aggregates. Unknown actions are ignored.
  pub fn fold(&mut self, value: &serde_json::Value) {
    let Some(action) = value.get("action").and_then(serde_json::Value::as_str) else {
      return;
    };
    match action {
      "SessionMeta" => self.fold_session_meta(value),
      "BuildStart" => self.on_build_start(),
      "BuildEnd" => self.build_end = Some(Instant::now()),
      "ModuleGraphReady" => self.fold_module_graph(value),
      "ChunkGraphReady" => self.fold_chunk_graph(value),
      "PackageGraphReady" => self.fold_package_graph(value),
      "AssetsReady" => self.fold_assets(value),
      "HookResolveIdCallStart" => self.hook_start(value, "resolveId"),
      "HookLoadCallStart" => self.hook_start(value, "load"),
      "HookTransformCallStart" => self.hook_start(value, "transform"),
      "HookRenderChunkStart" => self.hook_start(value, "renderChunk"),
      "HookResolveIdCallEnd" | "HookLoadCallEnd" | "HookTransformCallEnd"
      | "HookRenderChunkEnd" => self.hook_end(value),
      _ => {}
    }
  }

  fn on_build_start(&mut self) {
    // Reset per-build aggregates so the report reflects the most recent build, while keeping
    // session meta. Multiple builds in one session (e.g. `bundle.write()` twice) overwrite.
    let config = std::mem::take(&mut self.config);
    let (cwd, platform, format, input_count, plugin_count) = (
      self.cwd.take(),
      self.platform.take(),
      self.format.take(),
      self.input_count,
      self.plugin_count,
    );
    *self = Self::new(config);
    self.cwd = cwd;
    self.platform = platform;
    self.format = format;
    self.input_count = input_count;
    self.plugin_count = plugin_count;
    self.build_start = Some(Instant::now());
  }

  fn fold_session_meta(&mut self, value: &serde_json::Value) {
    if let Ok(meta) = serde_json::from_value::<MSessionMeta>(value.clone()) {
      self.cwd = Some(meta.cwd);
      self.platform = Some(meta.platform);
      self.format = Some(meta.format);
      self.input_count = meta.inputs.len();
      self.plugin_count = meta.plugins.len();
    }
  }

  fn fold_module_graph(&mut self, value: &serde_json::Value) {
    let Ok(graph) = serde_json::from_value::<MModuleGraph>(value.clone()) else {
      return;
    };
    self.module_count = graph.modules.len();
    self.external_count = 0;
    self.import_kind_hist.clear();
    let mut imported: Vec<(String, usize)> = Vec::new();
    for module in graph.modules {
      if module.is_external {
        self.external_count += 1;
      }
      if let Some(imports) = module.imports {
        for import in imports {
          *self.import_kind_hist.entry(import.kind).or_default() += 1;
        }
      }
      let importer_count = module.importers.map_or(0, |i| i.len());
      if importer_count > 0 {
        imported.push((module.id, importer_count));
      }
    }
    imported.sort_by(|a, b| b.1.cmp(&a.1));
    imported.truncate(self.config.top_n);
    self.most_imported = imported;
  }

  fn fold_chunk_graph(&mut self, value: &serde_json::Value) {
    let Ok(graph) = serde_json::from_value::<MChunkGraph>(value.clone()) else {
      return;
    };
    self.chunk_reason_hist.clear();
    self.chunks.clear();
    for chunk in graph.chunks {
      *self.chunk_reason_hist.entry(chunk.reason.clone()).or_default() += 1;
      let mut static_imports = Vec::new();
      let mut dynamic_imports = Vec::new();
      for import in chunk.imports {
        if import.kind == "dynamic-import" {
          dynamic_imports.push(import.chunk_id);
        } else {
          static_imports.push(import.chunk_id);
        }
      }
      self.chunks.insert(
        chunk.chunk_id,
        ChunkAgg {
          name: chunk.name.unwrap_or_else(|| format!("chunk-{}", chunk.chunk_id)),
          reason: chunk.reason,
          is_entry: chunk.is_user_defined_entry || chunk.is_async_entry,
          entry_module: chunk.entry_module,
          module_count: chunk.modules.len(),
          static_imports,
          dynamic_imports,
        },
      );
    }
  }

  fn fold_package_graph(&mut self, value: &serde_json::Value) {
    let Ok(graph) = serde_json::from_value::<MPackageGraph>(value.clone()) else {
      return;
    };
    self.package_direct = 0;
    self.package_transitive = 0;
    let mut packages: Vec<PackageAgg> = Vec::with_capacity(graph.packages.len());
    for package in graph.packages {
      if package.dependency_type == "direct" {
        self.package_direct += 1;
      } else {
        self.package_transitive += 1;
      }
      packages.push(PackageAgg {
        name: package.name.unwrap_or_else(|| "<unknown>".to_string()),
        version: package.version,
        dependency_type: package.dependency_type,
        size: u64::from(package.size),
        module_count: package.modules.len(),
        is_used: package.is_used,
      });
    }
    packages.sort_by(|a, b| b.size.cmp(&a.size));
    packages.truncate(self.config.top_n);
    self.packages = packages;
  }

  fn fold_assets(&mut self, value: &serde_json::Value) {
    let Ok(assets) = serde_json::from_value::<MAssets>(value.clone()) else {
      return;
    };
    self.asset_count = 0;
    self.total_bytes = 0;
    self.js_bytes = 0;
    self.css_bytes = 0;
    self.other_bytes = 0;
    self.chunk_sizes.clear();
    self.chunk_filenames.clear();
    let mut list: Vec<AssetAgg> = Vec::new();
    for asset in assets.assets {
      if asset.filename.ends_with(".map") {
        continue;
      }
      let size = u64::from(asset.size);
      self.asset_count += 1;
      self.total_bytes += size;
      if let Some(chunk_id) = asset.chunk_id {
        // Assets created from a chunk are the chunk's rendered JS.
        self.js_bytes += size;
        self.chunk_sizes.insert(chunk_id, size);
        self.chunk_filenames.insert(chunk_id, asset.filename.clone());
      } else if asset.filename.ends_with(".css") {
        self.css_bytes += size;
      } else {
        self.other_bytes += size;
      }
      list.push(AssetAgg { filename: asset.filename, size });
    }
    list.sort_by(|a, b| b.size.cmp(&a.size));
    list.truncate(self.config.top_n);
    self.assets = list;
  }

  fn hook_start(&mut self, value: &serde_json::Value, hook: &'static str) {
    let (Some(call_id), Some(plugin)) = (str_field(value, "call_id"), str_field(value, "plugin_name"))
    else {
      return;
    };
    self.pending.insert(
      call_id,
      PendingCall { plugin, hook, start: Instant::now(), module_id: str_field(value, "module_id") },
    );
  }

  fn hook_end(&mut self, value: &serde_json::Value) {
    let Some(call_id) = str_field(value, "call_id") else {
      return;
    };
    let Some(call) = self.pending.remove(&call_id) else {
      return;
    };
    let micros = u64::try_from(call.start.elapsed().as_micros()).unwrap_or(u64::MAX);
    let stat = self.hook_calls.entry((call.plugin, call.hook)).or_default();
    stat.count += 1;
    stat.micros += micros;
    if call.hook == "transform" {
      if let Some(module_id) = call.module_id {
        let entry = self.module_transform.entry(module_id).or_default();
        entry.count += 1;
        entry.micros += micros;
      }
    }
  }
}

fn str_field(value: &serde_json::Value, key: &str) -> Option<String> {
  value.get(key).and_then(serde_json::Value::as_str).map(ToString::to_string)
}

// --- minimal deserialize mirrors (serde ignores the action/build_id/session_id/content fields) ---

#[derive(Deserialize)]
struct MSessionMeta {
  #[serde(default)]
  inputs: Vec<serde_json::Value>,
  #[serde(default)]
  plugins: Vec<serde_json::Value>,
  #[serde(default)]
  cwd: String,
  #[serde(default)]
  platform: String,
  #[serde(default)]
  format: String,
}

#[derive(Deserialize)]
struct MModuleGraph {
  modules: Vec<MModule>,
}

#[derive(Deserialize)]
struct MModule {
  id: String,
  #[serde(default)]
  is_external: bool,
  #[serde(default)]
  imports: Option<Vec<MImport>>,
  #[serde(default)]
  importers: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct MImport {
  kind: String,
}

#[derive(Deserialize)]
struct MChunkGraph {
  chunks: Vec<MChunk>,
}

#[derive(Deserialize)]
struct MChunk {
  chunk_id: u32,
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  is_user_defined_entry: bool,
  #[serde(default)]
  is_async_entry: bool,
  #[serde(default)]
  entry_module: Option<String>,
  #[serde(default)]
  modules: Vec<String>,
  reason: String,
  #[serde(default)]
  imports: Vec<MChunkImport>,
}

#[derive(Deserialize)]
struct MChunkImport {
  chunk_id: u32,
  kind: String,
}

#[derive(Deserialize)]
struct MPackageGraph {
  packages: Vec<MPackage>,
}

#[derive(Deserialize)]
struct MPackage {
  #[serde(default)]
  name: Option<String>,
  #[serde(default)]
  version: Option<String>,
  #[serde(default)]
  is_used: bool,
  dependency_type: String,
  #[serde(default)]
  size: u32,
  #[serde(default)]
  modules: Vec<String>,
}

#[derive(Deserialize)]
struct MAssets {
  assets: Vec<MAsset>,
}

#[derive(Deserialize)]
struct MAsset {
  #[serde(default)]
  chunk_id: Option<u32>,
  #[serde(default)]
  size: u32,
  filename: String,
}
