use std::{ops::Range, sync::Arc};

use arcstr::ArcStr;
use rolldown_common::{Output, OutputChunk};
use rolldown_plugin::PluginContext;
use rolldown_plugin_utils::constants::ViteMetadata;
use rolldown_utils::rustc_hash::FxHashSetExt as _;
use rustc_hash::{FxHashMap, FxHashSet};
use string_wizard::MagicString;

use super::constant::{COMMENT_RE, IMPORT_RE};

pub fn overwrite_check_public_file(
  s: &mut MagicString<'_>,
  span: Range<usize>,
  value: String,
) -> anyhow::Result<()> {
  let src = &s.source().as_bytes()[span.start..span.end];
  let Some(start) = src
    .iter()
    .position(|&b| b == b'=')
    .and_then(|i| src[i + 1..].iter().position(|b| !b.is_ascii_whitespace()).map(|p| p + i + 1))
    .map(|pos| span.start + pos)
  else {
    return Err(anyhow::anyhow!("internal error, failed to overwrite attribute value"));
  };
  let pos = src[start - span.start];
  let wrap_offset = usize::from(pos == b'"' || pos == b'\'');
  s.update(start + wrap_offset, span.end - wrap_offset, value);
  Ok(())
}

pub fn is_excluded_url(url: &str) -> bool {
  url.starts_with('#')
    || {
      let b = url.as_bytes();
      if b.starts_with(b"//") {
        return true;
      }
      let mut i = 0;
      while i < b.len() && b[i].is_ascii_lowercase() {
        i += 1;
      }
      i > 0 && i + 2 < b.len() && &b[i..i + 3] == b"://"
    }
    || url.trim_start().get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:"))
}

pub fn is_entirely_import(code: &str) -> bool {
  // Only consider "side-effect" imports, which match <script type=module> semantics exactly
  // The regexes will remove too little in some exotic cases, but false-negatives are alright
  let without_imports = IMPORT_RE.replace_all(code, "");
  let without_comments = COMMENT_RE.replace_all(&without_imports, "");
  without_comments.trim().is_empty()
}

/// Represents an imported chunk or external module
#[derive(Debug, Clone)]
pub enum ImportedChunk {
  Chunk(Arc<OutputChunk>),
  External(ArcStr),
}

pub fn get_imported_chunks(chunk: &OutputChunk, bundle: &[Output]) -> Vec<ImportedChunk> {
  let mut seen = FxHashSet::with_capacity(bundle.len());
  let mut chunks = Vec::with_capacity(chunk.imports.len());
  get_imported_chunks_inner(chunk, bundle, &mut seen, &mut chunks);
  chunks
}

/// Recursively collects all imported chunks in post-order traversal
fn get_imported_chunks_inner(
  chunk: &OutputChunk,
  bundle: &[Output],
  seen: &mut FxHashSet<ArcStr>,
  chunks: &mut Vec<ImportedChunk>,
) {
  // TODO: we could improve below logic in future
  for file in &chunk.imports {
    // Find the importee in the bundle by filename
    let importee = bundle.iter().find_map(|output| match output {
      Output::Chunk(c) if c.filename == *file => Some(c),
      _ => None,
    });
    if let Some(importee) = importee {
      // If it's a chunk and we haven't seen it yet
      if !seen.contains(file) {
        seen.insert(file.clone());
        // Post-order traversal: first add all imports of this chunk
        get_imported_chunks_inner(importee, bundle, seen, chunks);
        // Then add the chunk itself
        chunks.push(ImportedChunk::Chunk(Arc::clone(importee)));
      }
    } else {
      // External import
      chunks.push(ImportedChunk::External(file.clone()));
    }
  }
}

pub fn get_css_files_for_chunk(
  ctx: &PluginContext,
  chunk: &Arc<OutputChunk>,
  bundle: &[rolldown_common::Output],
  analyzed_imported_css_files: &mut FxHashMap<ArcStr, Vec<String>>,
) -> Vec<String> {
  fn get_css_files_for_chunk_impl(
    ctx: &PluginContext,
    chunk: &Arc<OutputChunk>,
    bundle: &[rolldown_common::Output],
    analyzed_imported_css_files: &mut FxHashMap<ArcStr, Vec<String>>,
    seen_chunks: &mut FxHashSet<ArcStr>,
    seen_css: &mut FxHashSet<String>,
  ) -> Vec<String> {
    // Check if we've already processed this chunk (cycle detection)
    if seen_chunks.contains(&chunk.filename) {
      return Vec::new();
    }
    seen_chunks.insert(chunk.filename.clone());

    // Check if we've already analyzed this chunk (cache hit)
    if let Some(cached_files) = analyzed_imported_css_files.get(&chunk.filename) {
      let additionals = cached_files
        .iter()
        .filter(|file| !seen_css.contains(file.as_str()))
        .cloned()
        .collect::<Vec<_>>();
      for file in &additionals {
        seen_css.insert(file.clone());
      }
      return additionals;
    }

    let mut files = Vec::new();

    // Recursively collect CSS from imported chunks
    for import_filename in &chunk.imports {
      // TODO: we could improve below logic in future
      if let Some(importee) = bundle.iter().find_map(|output| match output {
        rolldown_common::Output::Chunk(c) if &c.filename == import_filename => Some(c),
        _ => None,
      }) {
        files.extend(get_css_files_for_chunk_impl(
          ctx,
          importee,
          bundle,
          analyzed_imported_css_files,
          seen_chunks,
          seen_css,
        ));
      }
    }

    // Cache the analyzed files for this chunk
    analyzed_imported_css_files.insert(chunk.filename.clone(), files.clone());

    // Collect CSS files from this chunk's metadata
    if let Some(chunk_metadata) = ctx
      .meta()
      .get::<ViteMetadata>()
      .and_then(|vite_metadata| vite_metadata.get(&chunk.preliminary_filename.as_str().into()))
    {
      for file in chunk_metadata.imported_css.iter() {
        if !seen_css.contains(file.as_str()) {
          seen_css.insert(file.to_string());
          files.push(file.to_string());
        }
      }
    }

    files
  }

  let mut seen_chunks = FxHashSet::default();
  let mut seen_css = FxHashSet::default();
  get_css_files_for_chunk_impl(
    ctx,
    chunk,
    bundle,
    analyzed_imported_css_files,
    &mut seen_chunks,
    &mut seen_css,
  )
}
