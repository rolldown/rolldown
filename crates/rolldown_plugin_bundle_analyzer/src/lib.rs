use std::{borrow::Cow, path::Path, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{EmittedAsset, Output, OutputChunk};
use rolldown_plugin::{HookNoopReturn, HookUsage, Plugin, PluginContext};
use rolldown_utils::rustc_hash::FxHashMapExt;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;
use sugar_path::SugarPath;

/// Plugin configuration
#[derive(Debug, Default)]
pub struct BundleAnalyzerPlugin {
  /// Output filename for the visualization data
  pub file_name: Option<String>,
}

/// Root data structure for chunk visualization
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnalyzeData {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) meta: Option<AnalyzeMeta>,
  pub(crate) chunks: Vec<ChunkData>,
  pub(crate) modules: Vec<ModuleData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnalyzeMeta {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) bundler: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) version: Option<String>,
  /// Milliseconds since Unix epoch (UTC)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) timestamp: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkData {
  pub(crate) id: String,
  pub(crate) name: String,
  pub(crate) size: usize,
  #[serde(rename = "type")]
  pub(crate) chunk_type: ChunkType,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) module_indices: Option<Vec<usize>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) entry_module: Option<usize>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) imports: Option<Vec<ImportRelation>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) reachable_module_indices: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ChunkType {
  StaticEntry,
  DynamicEntry,
  Common,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportRelation {
  pub(crate) target_chunk_index: usize,
  #[serde(rename = "type")]
  pub(crate) import_type: ImportType,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ImportType {
  Static,
  Dynamic,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModuleData {
  pub(crate) id: String,
  pub(crate) path: String,
  pub(crate) size: usize,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) importers: Option<Vec<usize>>,
}

impl Plugin for BundleAnalyzerPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:bundle-analyzer")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::GenerateBundle
  }

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    let analyze_data = self.build_analyze_data(ctx, args.bundle);
    let json = serde_json::to_string_pretty(&analyze_data)?;

    ctx
      .emit_file_async(EmittedAsset {
        file_name: Some(
          self.file_name.as_ref().map_or(arcstr::literal!("analyze-data.json"), ArcStr::from),
        ),
        source: json.into(),
        ..Default::default()
      })
      .await?;

    Ok(())
  }
}

impl BundleAnalyzerPlugin {
  fn build_analyze_data(&self, ctx: &PluginContext, bundle: &[Output]) -> AnalyzeData {
    let cwd = ctx.cwd();

    // Collect all chunks
    let chunks: Vec<&Arc<OutputChunk>> = bundle
      .iter()
      .filter_map(|output| match output {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .collect();

    // Build chunk filename to index mapping
    let chunk_filename_to_idx: FxHashMap<&str, usize> =
      chunks.iter().enumerate().map(|(idx, chunk)| (chunk.filename.as_str(), idx)).collect();

    // Collect all unique module IDs from all chunks and build module data
    let mut module_id_to_idx: FxHashMap<&str, usize> = FxHashMap::default();
    let mut modules_data: Vec<ModuleData> = Vec::new();

    for chunk in &chunks {
      for module_id in &chunk.module_ids {
        if !module_id_to_idx.contains_key(module_id.as_str()) {
          let idx = modules_data.len();
          module_id_to_idx.insert(module_id.as_str(), idx);

          // Get module info for size and other data
          let module_info = ctx.get_module_info(module_id.as_str());
          let size =
            module_info.as_ref().and_then(|info| info.code.as_ref().map(|c| c.len())).unwrap_or(0);

          modules_data.push(ModuleData {
            id: format!("mod-{idx}"),
            path: stabilize_module_id(module_id, cwd),
            size,
            importers: None, // Will be filled in later
          });
        }
      }
    }

    // Build module importers using ModuleInfo
    let mut module_importers: FxHashMap<usize, FxHashSet<usize>> =
      FxHashMap::with_capacity(modules_data.len());

    for (module_id, &module_idx) in &module_id_to_idx {
      if let Some(info) = ctx.get_module_info(module_id) {
        // Static importers
        for importer_id in &info.importers {
          if let Some(&importer_idx) = module_id_to_idx.get(importer_id.as_str()) {
            module_importers.entry(module_idx).or_default().insert(importer_idx);
          }
        }
        // Dynamic importers
        for importer_id in &info.dynamic_importers {
          if let Some(&importer_idx) = module_id_to_idx.get(importer_id.as_str()) {
            module_importers.entry(module_idx).or_default().insert(importer_idx);
          }
        }
      }
    }

    // Update modules with importer indices
    for (module_idx, module_data) in modules_data.iter_mut().enumerate() {
      if let Some(importers) = module_importers.get(&module_idx).filter(|i| !i.is_empty()) {
        let mut importers_vec: Vec<usize> = importers.iter().copied().collect();
        importers_vec.sort_unstable();
        module_data.importers = Some(importers_vec);
      }
    }

    // Build module dependency graph for reachability computation
    let module_dependencies = self.build_module_dependencies(ctx, &module_id_to_idx);

    // Build chunk data
    let mut chunks_data: Vec<ChunkData> = Vec::with_capacity(chunks.len());

    for (chunk_idx, chunk) in chunks.iter().enumerate() {
      let chunk_type = if chunk.is_entry {
        ChunkType::StaticEntry
      } else if chunk.is_dynamic_entry {
        ChunkType::DynamicEntry
      } else {
        ChunkType::Common
      };

      // Module indices in this chunk
      let module_indices: Vec<usize> = chunk
        .module_ids
        .iter()
        .filter_map(|id| module_id_to_idx.get(id.as_str()).copied())
        .collect();

      // Entry module index (for entry chunks)
      let entry_module =
        chunk.facade_module_id.as_ref().and_then(|id| module_id_to_idx.get(id.as_str()).copied());

      // Build import relations
      let mut imports: Vec<ImportRelation> = Vec::new();

      // Static imports
      for import_filename in &chunk.imports {
        if let Some(&target_idx) = chunk_filename_to_idx.get(import_filename.as_str())
          && target_idx != chunk_idx
        {
          imports.push(ImportRelation {
            target_chunk_index: target_idx,
            import_type: ImportType::Static,
          });
        }
      }

      // Dynamic imports
      for import_filename in &chunk.dynamic_imports {
        if let Some(&target_idx) = chunk_filename_to_idx.get(import_filename.as_str())
          && target_idx != chunk_idx
        {
          imports.push(ImportRelation {
            target_chunk_index: target_idx,
            import_type: ImportType::Dynamic,
          });
        }
      }

      // Compute reachable modules for entry chunks
      let reachable_module_indices = if chunk.is_entry || chunk.is_dynamic_entry {
        entry_module.map(|entry_idx| {
          let mut reachable = self.compute_reachable_modules(entry_idx, &module_dependencies);
          reachable.sort_unstable();
          reachable
        })
      } else {
        None
      };

      chunks_data.push(ChunkData {
        id: format!("chunk-{}", chunk.name),
        name: chunk.filename.to_string(),
        size: chunk.code.len(),
        chunk_type,
        module_indices: if module_indices.is_empty() { None } else { Some(module_indices) },
        entry_module,
        imports: if imports.is_empty() { None } else { Some(imports) },
        reachable_module_indices,
      });
    }

    AnalyzeData {
      meta: Some(AnalyzeMeta {
        bundler: Some("rolldown".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        timestamp: Some(epoch_millis()),
      }),
      chunks: chunks_data,
      modules: modules_data,
    }
  }

  /// Build module dependency graph: module_idx -> set of module indices it imports
  fn build_module_dependencies(
    &self,
    ctx: &PluginContext,
    module_id_to_idx: &FxHashMap<&str, usize>,
  ) -> FxHashMap<usize, FxHashSet<usize>> {
    let mut dependencies: FxHashMap<usize, FxHashSet<usize>> =
      FxHashMap::with_capacity(module_id_to_idx.len());

    for (module_id, &module_idx) in module_id_to_idx {
      if let Some(info) = ctx.get_module_info(module_id) {
        let mut deps = FxHashSet::default();

        // Static imports
        for imported_id in &info.imported_ids {
          if let Some(&imported_idx) = module_id_to_idx.get(imported_id.as_str()) {
            deps.insert(imported_idx);
          }
        }

        // Dynamic imports
        for imported_id in &info.dynamically_imported_ids {
          if let Some(&imported_idx) = module_id_to_idx.get(imported_id.as_str()) {
            deps.insert(imported_idx);
          }
        }

        if !deps.is_empty() {
          dependencies.insert(module_idx, deps);
        }
      }
    }

    dependencies
  }

  /// Compute all modules reachable from a given entry module
  fn compute_reachable_modules(
    &self,
    entry_module_idx: usize,
    dependencies: &FxHashMap<usize, FxHashSet<usize>>,
  ) -> Vec<usize> {
    let mut visited: FxHashSet<usize> = FxHashSet::default();
    let mut stack: Vec<usize> = vec![entry_module_idx];

    while let Some(module_idx) = stack.pop() {
      if visited.contains(&module_idx) {
        continue;
      }
      visited.insert(module_idx);

      if let Some(deps) = dependencies.get(&module_idx) {
        for &dep_idx in deps {
          if !visited.contains(&dep_idx) {
            stack.push(dep_idx);
          }
        }
      }
    }

    visited.into_iter().collect()
  }
}

/// Stabilize a module ID by converting absolute paths to relative paths from cwd.
/// This ensures stable, portable output across different machines.
fn stabilize_module_id(id: &str, cwd: &Path) -> String {
  let path = Path::new(id);
  if path.is_absolute() {
    // Convert absolute path to relative path from cwd using forward slashes
    path.relative(cwd).to_slash().map_or_else(|| id.to_string(), |s| s.to_string())
  } else if id.starts_with('\0') {
    // Escape virtual module prefix
    id.replace('\0', "\\0")
  } else {
    id.to_string()
  }
}

/// Get current timestamp as milliseconds since Unix epoch (UTC)
fn epoch_millis() -> u64 {
  use std::time::{SystemTime, UNIX_EPOCH};
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}
