use std::fmt::Write;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{AnalyzeData, ChunkType, ImportType, ModuleData};

/// Render the analyze data as LLM-friendly markdown (inspired by Bun's --metafile-md)
pub fn render_markdown(data: &AnalyzeData) -> String {
  let mut out = String::new();

  // Header
  out.push_str("# Bundle Analysis Report\n\n");
  out.push_str(
    "This report helps identify bundle size issues, dependency bloat, and optimization opportunities.\n\n",
  );

  // Table of Contents
  out.push_str("## Table of Contents\n\n");
  out.push_str("- [Quick Summary](#quick-summary)\n");
  out.push_str(
    "- [Largest Modules by Output Contribution](#largest-modules-by-output-contribution)\n",
  );
  out.push_str("- [Entry Point Analysis](#entry-point-analysis)\n");
  out.push_str("- [Dependency Chains](#dependency-chains)\n");
  out.push_str("- [Optimization Suggestions](#optimization-suggestions)\n");
  out.push_str("- [Full Module Graph](#full-module-graph)\n");
  out.push_str("- [Raw Data for Searching](#raw-data-for-searching)\n\n");
  out.push_str("---\n\n");

  // Quick Summary
  let total_output_size: usize = data.chunks.iter().map(|c| c.size).sum();
  let entry_count = data
    .chunks
    .iter()
    .filter(|c| matches!(c.chunk_type, ChunkType::StaticEntry | ChunkType::DynamicEntry))
    .count();
  let common_count =
    data.chunks.iter().filter(|c| matches!(c.chunk_type, ChunkType::Common)).count();

  out.push_str("## Quick Summary\n\n");
  out.push_str("| Metric | Value |\n");
  out.push_str("|--------|-------|\n");
  writeln!(out, "| Total output size | {} |", format_size(total_output_size)).unwrap();
  writeln!(out, "| Input modules | {} |", data.modules.len()).unwrap();
  writeln!(out, "| Entry points | {entry_count} |").unwrap();
  writeln!(out, "| Code-split chunks | {common_count} |").unwrap();
  out.push('\n');

  // Build reverse maps
  // module index → list of modules it imports (derived from importers)
  let mut module_imports: FxHashMap<usize, Vec<usize>> =
    FxHashMap::with_capacity_and_hasher(data.modules.len(), Default::default());
  for (idx, module) in data.modules.iter().enumerate() {
    if let Some(importers) = &module.importers {
      for &importer_idx in importers {
        module_imports.entry(importer_idx).or_default().push(idx);
      }
    }
  }

  // Largest Modules by Output Contribution
  render_largest_modules(&mut out, data, total_output_size);

  // Entry Point Analysis
  render_entry_point_analysis(&mut out, data);

  // Dependency Chains
  render_dependency_chains(&mut out, data);

  // Optimization Suggestions
  render_optimization_suggestions(&mut out, data);

  // Full Module Graph
  render_full_module_graph(&mut out, data, &module_imports);

  // Raw Data for Searching
  render_raw_data(&mut out, data, &module_imports);

  out
}

fn render_largest_modules(out: &mut String, data: &AnalyzeData, total_output_size: usize) {
  out.push_str("## Largest Modules by Output Contribution\n\n");
  out.push_str(
    "Modules sorted by bytes contributed to the output bundle. Large modules may indicate bloat.\n\n",
  );

  let mut all_modules: Vec<(usize, &ModuleData)> = data.modules.iter().enumerate().collect();
  all_modules.sort_by(|a, b| b.1.size.cmp(&a.1.size));

  out.push_str("| Output Bytes | % of Total | Module |\n");
  out.push_str("|--------------|------------|--------|\n");
  for &(_, module) in &all_modules {
    let pct = if total_output_size > 0 {
      (module.size as f64 / total_output_size as f64) * 100.0
    } else {
      0.0
    };
    writeln!(out, "| {} | {pct:.1}% | `{}` |", format_size(module.size), module.path).unwrap();
  }
  out.push('\n');
}

fn render_entry_point_analysis(out: &mut String, data: &AnalyzeData) {
  out.push_str("## Entry Point Analysis\n\n");
  out.push_str("Each entry point and the total code it loads (including shared chunks).\n\n");

  for chunk in &data.chunks {
    if !matches!(chunk.chunk_type, ChunkType::StaticEntry | ChunkType::DynamicEntry) {
      continue;
    }

    let entry_path = chunk.entry_module.and_then(|idx| data.modules.get(idx));

    if let Some(entry_module) = entry_path {
      writeln!(out, "### Entry: `{}`\n", entry_module.path).unwrap();
    } else {
      writeln!(out, "### Entry: `{}`\n", chunk.name).unwrap();
    }

    writeln!(out, "**Output file**: `{}`", chunk.name).unwrap();
    writeln!(out, "**Bundle size**: {}", format_size(chunk.size)).unwrap();

    // Show chunk imports (code-splitting)
    if let Some(imports) = &chunk.imports
      && !imports.is_empty()
    {
      out.push('\n');
      out.push_str("**Loads these chunks** (code-splitting):\n");
      for import in imports {
        if let Some(target) = data.chunks.get(import.target_chunk_index) {
          let kind = match import.import_type {
            ImportType::Static => "import-statement",
            ImportType::Dynamic => "dynamic-import",
          };
          writeln!(out, "- `{}` ({}, {kind})", target.name, format_size(target.size)).unwrap();
        }
      }
    }

    // Bundled modules in this entry
    if let Some(indices) = &chunk.module_indices
      && !indices.is_empty()
    {
      let mut sorted_indices: Vec<usize> = indices.clone();
      sorted_indices.sort_by(|a, b| {
        let size_a = data.modules.get(*a).map_or(0, |m| m.size);
        let size_b = data.modules.get(*b).map_or(0, |m| m.size);
        size_b.cmp(&size_a)
      });

      out.push_str("\n**Bundled modules** (sorted by contribution):\n\n");
      out.push_str("| Bytes | Module |\n");
      out.push_str("|-------|--------|\n");
      for &idx in &sorted_indices {
        if let Some(module) = data.modules.get(idx) {
          writeln!(out, "| {} | `{}` |", format_size(module.size), module.path).unwrap();
        }
      }
    }

    out.push('\n');
  }
}

fn render_dependency_chains(out: &mut String, data: &AnalyzeData) {
  out.push_str("## Dependency Chains\n\n");
  out.push_str(
    "For each module, shows what files import it. Use this to understand why a module is included.\n\n",
  );

  // Find modules imported by multiple files
  let mut multi_imported: Vec<(usize, &ModuleData)> = data
    .modules
    .iter()
    .enumerate()
    .filter(|(_, m)| m.importers.as_ref().is_some_and(|i| i.len() >= 2))
    .collect();
  multi_imported.sort_by(|a, b| {
    let count_a = a.1.importers.as_ref().map_or(0, |i| i.len());
    let count_b = b.1.importers.as_ref().map_or(0, |i| i.len());
    count_b.cmp(&count_a)
  });

  if !multi_imported.is_empty() {
    out.push_str("### Most Commonly Imported Modules\n\n");
    out.push_str("Modules imported by many files. Extracting these to shared chunks may help.\n\n");
    out.push_str("| Import Count | Module | Imported By |\n");
    out.push_str("|--------------|--------|-------------|\n");
    for &(_, module) in &multi_imported {
      if let Some(importers) = &module.importers {
        let importer_paths: Vec<&str> =
          importers.iter().filter_map(|&i| data.modules.get(i).map(|m| m.path.as_str())).collect();
        writeln!(
          out,
          "| {} | `{}` | {} |",
          importers.len(),
          module.path,
          importer_paths.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ")
        )
        .unwrap();
      }
    }
    out.push('\n');
  }
}

fn render_optimization_suggestions(out: &mut String, data: &AnalyzeData) {
  // Step 1: Collect static entry chunks with reachability sets
  let static_entries: Vec<(usize, &str, FxHashSet<usize>)> = data
    .chunks
    .iter()
    .enumerate()
    .filter(|(_, c)| matches!(c.chunk_type, ChunkType::StaticEntry))
    .filter_map(|(idx, c)| {
      let reachable = c.reachable_module_indices.as_ref()?;
      let entry_path = c
        .entry_module
        .and_then(|i| data.modules.get(i))
        .map_or(c.name.as_str(), |m| m.path.as_str());
      Some((idx, entry_path, reachable.iter().copied().collect::<FxHashSet<usize>>()))
    })
    .collect();

  if static_entries.is_empty() {
    return;
  }

  // Step 2: For each common chunk, find modules reachable by exactly one static entry
  struct Suggestion<'a> {
    common_chunk_name: &'a str,
    common_chunk_total_module_size: usize,
    entry_path: &'a str,
    modules: Vec<(&'a str, usize)>, // (path, size)
    total_size: usize,
  }

  let mut suggestions: Vec<Suggestion> = Vec::new();

  for common_chunk in data.chunks.iter().filter(|c| matches!(c.chunk_type, ChunkType::Common)) {
    let module_indices = match &common_chunk.module_indices {
      Some(indices) if !indices.is_empty() => indices,
      _ => continue,
    };

    // Only consider common chunks shared by multiple static entries
    let reaching_entry_count = static_entries
      .iter()
      .filter(|(_, _, reachable)| module_indices.iter().any(|idx| reachable.contains(idx)))
      .count();
    if reaching_entry_count < 2 {
      continue;
    }

    // Total source size of all modules in this common chunk
    let common_chunk_total_module_size: usize =
      module_indices.iter().filter_map(|&i| data.modules.get(i)).map(|m| m.size).sum();

    // Group modules by the single static entry that reaches them
    let mut by_entry: FxHashMap<usize, Vec<usize>> = FxHashMap::default();

    for &mod_idx in module_indices {
      let mut reaching_entries: Vec<usize> = Vec::new();
      for &(entry_idx, _, ref reachable) in &static_entries {
        if reachable.contains(&mod_idx) {
          reaching_entries.push(entry_idx);
        }
      }
      if reaching_entries.len() == 1 {
        by_entry.entry(reaching_entries[0]).or_default().push(mod_idx);
      }
    }

    for (entry_idx, mod_indices) in by_entry {
      let entry_path = static_entries
        .iter()
        .find(|(idx, _, _)| *idx == entry_idx)
        .map(|(_, path, _)| *path)
        .unwrap_or("unknown");

      let mut modules: Vec<(&str, usize)> = mod_indices
        .iter()
        .filter_map(|&i| data.modules.get(i).map(|m| (m.path.as_str(), m.size)))
        .collect();
      modules.sort_by(|a, b| b.1.cmp(&a.1));

      let total_size: usize = modules.iter().map(|(_, s)| *s).sum();

      suggestions.push(Suggestion {
        common_chunk_name: &common_chunk.name,
        common_chunk_total_module_size,
        entry_path,
        modules,
        total_size,
      });
    }
  }

  if suggestions.is_empty() {
    return;
  }

  suggestions.sort_by(|a, b| b.total_size.cmp(&a.total_size));

  // Step 3: Render
  out.push_str("## Optimization Suggestions\n\n");
  out.push_str("Actionable suggestions to improve bundle efficiency.\n\n");

  for suggestion in &suggestions {
    let pct = if suggestion.common_chunk_total_module_size > 0 {
      (suggestion.total_size as f64 / suggestion.common_chunk_total_module_size as f64) * 100.0
    } else {
      0.0
    };

    let level = if pct > 50.0 {
      "HIGH"
    } else if pct >= 30.0 {
      "MEDIUM"
    } else {
      "LOW"
    };

    writeln!(
      out,
      "### [{level}] Common chunk `{}`: {pct:.1}% only reachable from `{}`\n",
      suggestion.common_chunk_name, suggestion.entry_path,
    )
    .unwrap();

    writeln!(
      out,
      "**{} modules** ({} of {}) in common chunk `{}` are only reachable \
       from entry `{}`. Consider adjusting code splitting configuration to move these \
       modules closer to their entry point.\n",
      suggestion.modules.len(),
      format_size(suggestion.total_size),
      format_size(suggestion.common_chunk_total_module_size),
      suggestion.common_chunk_name,
      suggestion.entry_path,
    )
    .unwrap();

    out.push_str("| Size | Module |\n");
    out.push_str("|------|--------|\n");
    for &(path, size) in &suggestion.modules {
      writeln!(out, "| {} | `{path}` |", format_size(size)).unwrap();
    }
    out.push('\n');
  }

  out.push_str("### Tip: Enable `entriesAware` for smarter code splitting\n\n");
  out.push_str(
    "Consider enabling `entriesAware: true` in your `codeSplitting.groups` configuration \
     to let rolldown automatically split chunks based on entry point reachability. \
     See https://rolldown.rs/reference/TypeAlias.CodeSplittingGroup#entriesaware\n\n",
  );
}

fn render_full_module_graph(
  out: &mut String,
  data: &AnalyzeData,
  module_imports: &FxHashMap<usize, Vec<usize>>,
) {
  out.push_str("## Full Module Graph\n\n");
  out.push_str("Complete dependency information for each module.\n\n");

  // Sort modules alphabetically by path for consistent output
  let mut sorted_modules: Vec<(usize, &ModuleData)> = data.modules.iter().enumerate().collect();
  sorted_modules.sort_by(|a, b| a.1.path.cmp(&b.1.path));

  for &(idx, module) in &sorted_modules {
    writeln!(out, "### `{}`\n", module.path).unwrap();
    writeln!(out, "- **Output contribution**: {}", format_size(module.size)).unwrap();

    // Imported by
    if let Some(importers) = &module.importers
      && !importers.is_empty()
    {
      let importer_paths: Vec<String> = importers
        .iter()
        .filter_map(|&i| data.modules.get(i).map(|m| format!("`{}`", m.path)))
        .collect();
      writeln!(out, "- **Imported by** ({} files): {}", importers.len(), importer_paths.join(" "))
        .unwrap();
    } else {
      out.push_str("- **Imported by**: (entry point or orphan)\n");
    }

    // Imports
    if let Some(deps) = module_imports.get(&idx)
      && !deps.is_empty()
    {
      out.push_str("- **Imports**:\n");
      for &dep_idx in deps {
        if let Some(dep) = data.modules.get(dep_idx) {
          writeln!(out, "  - `{}`", dep.path).unwrap();
        }
      }
    }

    out.push('\n');
  }
}

fn render_raw_data(
  out: &mut String,
  data: &AnalyzeData,
  module_imports: &FxHashMap<usize, Vec<usize>>,
) {
  out.push_str("## Raw Data for Searching\n\n");
  out.push_str("This section contains raw, grep-friendly data. Use these patterns:\n");
  out.push_str("- `[MODULE:` - Find all modules\n");
  out.push_str("- `[OUTPUT_BYTES:` - Find output contribution for each module\n");
  out.push_str("- `[IMPORT:` - Find all import relationships\n");
  out.push_str("- `[IMPORTED_BY:` - Find reverse dependencies\n");
  out.push_str("- `[ENTRY:` - Find entry points\n");
  out.push_str("- `[CHUNK:` - Find code-split chunks\n\n");

  // All Modules
  let mut all_modules: Vec<(usize, &ModuleData)> = data.modules.iter().enumerate().collect();
  all_modules.sort_by(|a, b| b.1.size.cmp(&a.1.size));

  out.push_str("### All Modules\n\n```\n");
  for &(_, module) in &all_modules {
    writeln!(out, "[MODULE: {}]", module.path).unwrap();
    writeln!(out, "[OUTPUT_BYTES: {} = {} bytes]", module.path, module.size).unwrap();
  }
  out.push_str("```\n\n");

  // All Imports
  let mut sorted_modules: Vec<(usize, &ModuleData)> = data.modules.iter().enumerate().collect();
  sorted_modules.sort_by(|a, b| a.1.path.cmp(&b.1.path));

  out.push_str("### All Imports\n\n```\n");
  for &(idx, module) in &sorted_modules {
    if let Some(deps) = module_imports.get(&idx) {
      for &dep_idx in deps {
        if let Some(dep) = data.modules.get(dep_idx) {
          writeln!(out, "[IMPORT: {} -> {}]", module.path, dep.path).unwrap();
        }
      }
    }
  }
  out.push_str("```\n\n");

  // Reverse Dependencies
  out.push_str("### Reverse Dependencies (Imported By)\n\n```\n");
  for &(_, module) in &sorted_modules {
    if let Some(importers) = &module.importers {
      for &importer_idx in importers {
        if let Some(importer) = data.modules.get(importer_idx) {
          writeln!(out, "[IMPORTED_BY: {} <- {}]", module.path, importer.path).unwrap();
        }
      }
    }
  }
  out.push_str("```\n\n");

  // Entry Points
  out.push_str("### Entry Points\n\n```\n");
  for chunk in &data.chunks {
    if matches!(chunk.chunk_type, ChunkType::StaticEntry | ChunkType::DynamicEntry) {
      let entry_path = chunk
        .entry_module
        .and_then(|idx| data.modules.get(idx))
        .map_or("unknown", |m| m.path.as_str());
      writeln!(out, "[ENTRY: {} -> {} ({} bytes)]", entry_path, chunk.name, chunk.size).unwrap();
    }
  }
  out.push_str("```\n\n");

  // Chunks
  out.push_str("### Chunks\n\n```\n");
  for chunk in &data.chunks {
    if matches!(chunk.chunk_type, ChunkType::Common) {
      writeln!(out, "[CHUNK: {} ({} bytes)]", chunk.name, chunk.size).unwrap();
    }
  }
  out.push_str("```\n");
}

/// Format a byte size into a human-readable string
fn format_size(bytes: usize) -> String {
  if bytes < 1024 {
    format!("{bytes} B")
  } else if bytes < 1024 * 1024 {
    format!("{:.1} kB", bytes as f64 / 1024.0)
  } else {
    format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
  }
}
