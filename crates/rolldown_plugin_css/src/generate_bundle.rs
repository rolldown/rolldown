use std::path::Path;
use std::sync::Arc;

use arcstr::ArcStr;
use regex::Regex;
use rolldown_common::{EmittedAsset, Output, StrOrBytes};
use rolldown_plugin::PluginContext;
use rustc_hash::FxHashSet;

use crate::{AccumulatedCss, PureCssChunks};

/// Remove pure CSS JS chunks from the bundle and clean up import references
/// in remaining chunks.
///
/// A "pure CSS chunk" is one where all modules are CSS (no JS exports). These
/// chunks exist only as vehicles for their CSS side-effects and should be
/// removed from the JS output once the CSS has been emitted as a separate asset.
pub fn prune_pure_css_chunks(
  ctx: &PluginContext,
  args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
) {
  let Some(pure_css_chunks) = ctx.meta().get::<PureCssChunks>() else {
    return;
  };

  if pure_css_chunks.inner.is_empty() {
    return;
  }

  // Map preliminary filenames (stored during render_chunk) to final filenames
  // in the output bundle.
  let mut pure_css_filenames: FxHashSet<ArcStr> = FxHashSet::default();
  for output in args.bundle.iter() {
    if let Output::Chunk(chunk) = output {
      if pure_css_chunks.inner.contains(chunk.preliminary_filename.as_str()) {
        pure_css_filenames.insert(chunk.filename.clone());
      }
    }
  }

  if pure_css_filenames.is_empty() {
    return;
  }

  // Build a regex to match import statements that reference pure CSS chunks.
  // We match against the basename of the filename (e.g. "style.js") since
  // import paths may use relative segments.
  let escaped_basenames: Vec<String> = pure_css_filenames
    .iter()
    .filter_map(|file| {
      Path::new(file.as_str()).file_name().and_then(|v| v.to_str().map(regex::escape))
    })
    .collect();

  if escaped_basenames.is_empty() {
    return;
  }

  let pattern = escaped_basenames.join("|");

  let import_re = if args.options.format.is_esm() {
    // Match: import "./chunk-abc.js";  or  import './chunk-abc.js';
    Regex::new(&format!(r#"\bimport\s*["'][^"']*(?:{pattern})["'];"#))
  } else {
    // Match: require("./chunk-abc.js");  or  require('./chunk-abc.js');
    Regex::new(&format!(r#"(\b|,\s*)require\(\s*["'`][^"'`]*(?:{pattern})["'`]\)(;|,)"#))
  };

  let Ok(import_re) = import_re else {
    return;
  };

  // Rewrite remaining chunks: remove import statements and filter `imports` arrays
  for output in args.bundle.iter_mut() {
    if let Output::Chunk(chunk) = output {
      let imports_pure_css = chunk.imports.iter().any(|f| pure_css_filenames.contains(f));
      if !imports_pure_css {
        continue;
      }

      let mut new_chunk = (**chunk).clone();

      // Remove pure CSS chunk filenames from the imports array
      new_chunk.imports.retain(|file| !pure_css_filenames.contains(file));

      // Remove or replace import statements in the code
      if args.options.format.is_esm() {
        new_chunk.code = import_re
          .replace_all(&chunk.code, |captures: &regex::Captures<'_>| {
            let len = captures.get(0).unwrap().len();
            format!("/* empty css {:<width$}*/", "", width = len.saturating_sub(15))
          })
          .into_owned();
      } else {
        new_chunk.code = import_re
          .replace_all(&chunk.code, |captures: &regex::Captures<'_>| {
            let len = captures.get(0).unwrap().len();
            if let Some(p2) = captures.get(2)
              && p2.as_str() == ";"
            {
              return format!(";/* empty css {:<width$}*/", "", width = len.saturating_sub(16));
            }
            let p1 = captures.get(1).map_or("", |m| m.as_str());
            format!("{p1}/* empty css {:<width$}*/", "", width = len.saturating_sub(15 + p1.len()))
          })
          .into_owned();
      }

      *chunk = Arc::new(new_chunk);
    }
  }

  // Remove the pure CSS chunks (and their sourcemap assets) from the bundle
  args.bundle.retain(|output| match output {
    Output::Chunk(chunk) => !pure_css_filenames.contains(&chunk.filename),
    Output::Asset(asset) => {
      // Also remove sourcemap files for pure CSS chunks (e.g. "style.js.map")
      !pure_css_filenames
        .iter()
        .any(|name| asset.filename.as_str() == format!("{name}.map").as_str())
    }
  });
}

/// Collect CSS from all chunks in deterministic order and emit a single `style.css` asset.
///
/// Ordering algorithm (matches Vite):
/// 1. Entry chunks first, with their static imports traversed depth-first
/// 2. Dynamic import chunks next, also depth-first
/// 3. Deduplication: skip chunks already visited
///
/// This ensures that CSS from statically-imported modules (lower specificity, loaded
/// unconditionally) appears before dynamically-imported modules (higher specificity).
pub fn emit_single_css_bundle(
  ctx: &PluginContext,
  args: &rolldown_plugin::HookGenerateBundleArgs<'_>,
) -> anyhow::Result<()> {
  let Some(accumulated) = ctx.meta().get::<AccumulatedCss>() else {
    return Ok(());
  };

  let accumulated_entries = accumulated.inner.lock().clone();
  if accumulated_entries.is_empty() {
    return Ok(());
  }

  // Build a map from preliminary_filename → css_content
  let css_by_preliminary: rustc_hash::FxHashMap<&str, &str> =
    accumulated_entries.iter().map(|(filename, css)| (filename.as_str(), css.as_str())).collect();

  // Collect ordered chunk preliminary filenames by traversing the bundle
  let ordered_filenames = collect_css_in_chunk_order(args.bundle);

  // Concatenate CSS in the determined order
  let mut css_parts: Vec<&str> = Vec::new();
  for preliminary_filename in &ordered_filenames {
    if let Some(css) = css_by_preliminary.get(preliminary_filename.as_str()) {
      if !css.is_empty() {
        css_parts.push(css);
      }
    }
  }

  if css_parts.is_empty() {
    return Ok(());
  }

  let combined_css = css_parts.join("\n");

  ctx.emit_file(
    EmittedAsset {
      name: Some("style.css".to_owned()),
      original_file_name: None,
      file_name: Some(ArcStr::from("style.css")),
      source: StrOrBytes::Str(combined_css),
    },
    None,
    None,
  )?;

  Ok(())
}

/// Traverse bundle chunks in deterministic order and return their preliminary filenames.
///
/// Order: entry chunks first (with static imports depth-first), then dynamic chunks.
fn collect_css_in_chunk_order(bundle: &[Output]) -> Vec<ArcStr> {
  use rustc_hash::FxHashMap;

  // Build lookup maps from filename → chunk data
  let mut chunk_by_filename: FxHashMap<&str, &rolldown_common::OutputChunk> = FxHashMap::default();
  let mut entry_chunks: Vec<&rolldown_common::OutputChunk> = Vec::new();

  for output in bundle {
    if let Output::Chunk(chunk) = output {
      chunk_by_filename.insert(chunk.filename.as_str(), chunk);
      if chunk.is_entry {
        entry_chunks.push(chunk);
      }
    }
  }

  // Sort entry chunks by name for deterministic ordering
  entry_chunks.sort_by(|a, b| a.name.cmp(&b.name));

  let mut visited: FxHashSet<ArcStr> = FxHashSet::default();
  let mut ordered: Vec<ArcStr> = Vec::new();

  // Phase 1: Entry chunks and their static imports (depth-first)
  for entry in &entry_chunks {
    visit_chunk_depth_first(entry, &chunk_by_filename, &mut visited, &mut ordered, false);
  }

  // Phase 2: Dynamic imports — traverse any remaining unvisited chunks
  // We re-iterate entries to pick up dynamic imports from already-visited chunks.
  for entry in &entry_chunks {
    visit_dynamic_imports(entry, &chunk_by_filename, &mut visited, &mut ordered);
  }

  ordered
}

/// Depth-first traversal of a chunk's static imports, then the chunk itself.
fn visit_chunk_depth_first(
  chunk: &rolldown_common::OutputChunk,
  chunk_by_filename: &rustc_hash::FxHashMap<&str, &rolldown_common::OutputChunk>,
  visited: &mut FxHashSet<ArcStr>,
  ordered: &mut Vec<ArcStr>,
  include_dynamic: bool,
) {
  let preliminary = ArcStr::from(&chunk.preliminary_filename);
  if visited.contains(&preliminary) {
    return;
  }
  visited.insert(preliminary.clone());

  // Visit static imports first (depth-first)
  for import_filename in &chunk.imports {
    if let Some(imported_chunk) = chunk_by_filename.get(import_filename.as_str()) {
      visit_chunk_depth_first(imported_chunk, chunk_by_filename, visited, ordered, include_dynamic);
    }
  }

  // Then add this chunk
  ordered.push(preliminary);

  // Optionally visit dynamic imports
  if include_dynamic {
    for dyn_filename in &chunk.dynamic_imports {
      if let Some(dyn_chunk) = chunk_by_filename.get(dyn_filename.as_str()) {
        visit_chunk_depth_first(dyn_chunk, chunk_by_filename, visited, ordered, true);
      }
    }
  }
}

/// Visit dynamic imports from all already-ordered chunks.
fn visit_dynamic_imports(
  chunk: &rolldown_common::OutputChunk,
  chunk_by_filename: &rustc_hash::FxHashMap<&str, &rolldown_common::OutputChunk>,
  visited: &mut FxHashSet<ArcStr>,
  ordered: &mut Vec<ArcStr>,
) {
  // Process dynamic imports from this chunk
  for dyn_filename in &chunk.dynamic_imports {
    if let Some(dyn_chunk) = chunk_by_filename.get(dyn_filename.as_str()) {
      let preliminary = ArcStr::from(&dyn_chunk.preliminary_filename);
      if !visited.contains(&preliminary) {
        visit_chunk_depth_first(dyn_chunk, chunk_by_filename, visited, ordered, true);
      }
    }
  }

  // Also visit dynamic imports from static imports
  for import_filename in &chunk.imports {
    if let Some(imported_chunk) = chunk_by_filename.get(import_filename.as_str()) {
      visit_dynamic_imports(imported_chunk, chunk_by_filename, visited, ordered);
    }
  }
}
