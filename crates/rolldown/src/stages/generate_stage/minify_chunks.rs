use std::{collections::BTreeMap, sync::OnceLock};

use oxc::{
  codegen::{self, CodegenOptions, CommentOptions},
  minifier::{ManglePropertyCache, MinifierOptions as OxcMinifierOptions},
  span::SourceType,
};
use oxc_allocator::AllocatorPool;
use rolldown_common::{
  InstantiatedChunk, InstantiationKind, MinifyOptions, NormalizedBundlerOptions, Output,
};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::{BuildDiagnostic, BuildResult, InvalidOptionType};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps};

use crate::type_alias::IndexInstantiatedChunks;

use super::GenerateStage;

/// Everything needed to minify ECMAScript chunks, computed once per build. It owns its options,
/// so `render_chunks` can hand single chunks to the rayon pool as soon as their `renderChunk`
/// hooks return, instead of waiting for every chunk's hooks before minifying any of them.
pub struct ChunkMinifier {
  compress: bool,
  minify_options: OxcMinifierOptions,
  codegen_options: CodegenOptions,
  source_type: SourceType,
  allocator_pool: AllocatorPool,
  property_mangle_cache: OnceLock<ManglePropertyCache>,
}

impl ChunkMinifier {
  /// Returns `None` when minification is disabled.
  pub fn new(
    options: &NormalizedBundlerOptions,
    chunks: &IndexInstantiatedChunks,
  ) -> BuildResult<Option<Self>> {
    let Some((compress, minify_options, remove_whitespace, ascii_only)) =
      chunk_minify_options(&options.minify)
    else {
      return Ok(None);
    };
    if minify_options.mangle_properties.is_some()
      && chunks.iter().filter(|chunk| is_minifiable_ecma_chunk(chunk)).take(2).count() > 1
    {
      return Err(
        BuildDiagnostic::invalid_option(InvalidOptionType::ManglePropertiesWithMultipleChunks)
          .into(),
      );
    }
    let codegen_options = CodegenOptions {
      minify: remove_whitespace,
      ascii_only: options.charset.is_ascii() || ascii_only,
      comments: CommentOptions {
        normal: !remove_whitespace,
        jsdoc: options.comments.jsdoc && !remove_whitespace,
        annotation: options.comments.annotation,
        legal: if options.comments.legal || !remove_whitespace {
          codegen::LegalComment::Inline
        } else {
          codegen::LegalComment::None
        },
      },
      ..CodegenOptions::default()
    };
    Ok(Some(Self {
      compress,
      minify_options: minify_options.clone(),
      codegen_options,
      source_type: options.format.source_type().with_jsx(true),
      allocator_pool: AllocatorPool::new(rayon::current_num_threads()),
      property_mangle_cache: OnceLock::new(),
    }))
  }

  pub fn should_minify(chunk: &InstantiatedChunk) -> bool {
    is_minifiable_ecma_chunk(chunk)
  }

  /// Minifies one chunk's code, returning the new code and its map chained onto `map`.
  pub fn minify(
    &self,
    content: &str,
    map: Option<&SourceMap>,
    filename: &str,
  ) -> BuildResult<(String, Option<SourceMap>)> {
    let allocator_guard = self.allocator_pool.get();
    let (minified_content, new_map, chunk_property_mangle_cache) = EcmaCompiler::dce_or_minify(
      &allocator_guard,
      content,
      self.source_type,
      map.is_some(),
      filename,
      self.compress,
      self.minify_options.clone(),
      self.codegen_options.clone(),
    )?;
    if let Some(cache) = chunk_property_mangle_cache {
      self
        .property_mangle_cache
        .set(cache)
        .expect("property mangling is limited to one JavaScript chunk");
    }
    let collapsed_map = match (map, &new_map) {
      (Some(origin_map), Some(new_map)) => Some(collapse_sourcemaps(&[origin_map, new_map])),
      _ => None,
    };
    Ok((minified_content, collapsed_map))
  }

  pub fn into_mangle_cache(self) -> Option<BTreeMap<String, Option<String>>> {
    self.property_mangle_cache.into_inner().map(|cache| {
      cache
        .into_iter()
        .map(|(original, target)| {
          (original.into_string(), target.map(oxc_str::CompactStr::into_string))
        })
        .collect()
    })
  }
}

impl GenerateStage<'_> {
  pub(crate) fn validate_mangle_properties_output(
    options: &NormalizedBundlerOptions,
    outputs: &[Output],
  ) -> BuildResult<()> {
    let Some((_, minify_options, _, _)) = chunk_minify_options(&options.minify) else {
      return Ok(());
    };
    if minify_options.mangle_properties.is_some()
      && outputs
        .iter()
        .filter(|output| {
          matches!(output, Output::Chunk(chunk) if !test_d_ts_pattern(chunk.filename.as_str()))
        })
        .take(2)
        .count()
        > 1
    {
      return Err(
        BuildDiagnostic::invalid_option(InvalidOptionType::ManglePropertiesWithMultipleChunks)
          .into(),
      );
    }
    Ok(())
  }
}

fn chunk_minify_options(
  options: &MinifyOptions,
) -> Option<(bool, &OxcMinifierOptions, bool, bool)> {
  match options {
    MinifyOptions::Disabled => None,
    MinifyOptions::DeadCodeEliminationOnly(options) => Some((false, options, false, false)),
    MinifyOptions::Enabled(options) => {
      Some((true, &options.options, options.remove_whitespace, options.ascii_only))
    }
  }
}

fn is_minifiable_ecma_chunk(chunk: &InstantiatedChunk) -> bool {
  matches!(chunk.kind, InstantiationKind::Ecma(_))
    && !test_d_ts_pattern(chunk.preliminary_filename.as_str())
}

fn test_d_ts_pattern(input: &str) -> bool {
  input.ends_with(".d.ts") || input.ends_with(".d.cts") || input.ends_with(".d.mts")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_edge_cases() {
    assert!(test_d_ts_pattern(".d.ts"));
    assert!(test_d_ts_pattern(".d.cts"));
    assert!(test_d_ts_pattern(".d.mts"));
  }

  #[test]
  fn test_invalid_patterns_wrong_extension() {
    assert!(!test_d_ts_pattern(".d.tsx"));
    assert!(!test_d_ts_pattern(".d.ctsx"));
    assert!(!test_d_ts_pattern(".d.mtsx"));
    assert!(!test_d_ts_pattern(".d.cjs"));
  }

  #[test]
  fn test_invalid_patterns_missing_d() {
    assert!(!test_d_ts_pattern(".c.ts"));
    assert!(!test_d_ts_pattern(".m.ts"));
    assert!(!test_d_ts_pattern("abc.ts"));
    assert!(!test_d_ts_pattern("d.ts"));
  }

  #[test]
  fn test_invalid_patterns_extra_chars() {
    assert!(!test_d_ts_pattern(".da.ts"));
    assert!(!test_d_ts_pattern(".d.ats"));
    assert!(!test_d_ts_pattern(".d.tsa"));
  }

  #[test]
  fn test_invalid_patterns_short_input() {
    assert!(!test_d_ts_pattern(".d"));
    assert!(!test_d_ts_pattern(".t"));
    assert!(!test_d_ts_pattern("."));
    assert!(!test_d_ts_pattern(""));
    assert!(!test_d_ts_pattern(".ts")); // added test
  }
}
