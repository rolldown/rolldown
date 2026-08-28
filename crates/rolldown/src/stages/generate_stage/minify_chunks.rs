use std::{collections::BTreeMap, sync::OnceLock};

use oxc::{
  codegen::{self, CodegenOptions, CommentOptions},
  minifier::MinifierOptions as OxcMinifierOptions,
};
use oxc_allocator::AllocatorPool;
use rolldown_common::{
  InstantiatedChunk, InstantiationKind, MinifyOptions, NormalizedBundlerOptions, Output,
};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::{BuildDiagnostic, BuildResult, InvalidOptionType};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexInstantiatedChunks;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn minify_chunks(
    options: &NormalizedBundlerOptions,
    chunks: &mut IndexInstantiatedChunks,
  ) -> BuildResult<Option<BTreeMap<String, Option<String>>>> {
    let Some((compress, minify_options, remove_whitespace)) = chunk_minify_options(&options.minify)
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
    let allocator_pool = AllocatorPool::new(rayon::current_num_threads());
    let source_type = options.format.source_type().with_jsx(true);

    let property_mangle_cache = OnceLock::new();
    chunks.par_iter_mut().try_for_each(|chunk| -> anyhow::Result<()> {
      if !is_minifiable_ecma_chunk(chunk) {
        return Ok(());
      }
      let codegen_options = CodegenOptions {
        minify: remove_whitespace,
        comments: CommentOptions {
          normal: !remove_whitespace,
          jsdoc: options.comments.jsdoc && !remove_whitespace,
          annotation: options.comments.annotation && !remove_whitespace,
          legal: if options.comments.legal || !remove_whitespace {
            codegen::LegalComment::Inline
          } else {
            codegen::LegalComment::None
          },
        },
        ..CodegenOptions::default()
      };

      let allocator_guard = allocator_pool.get();
      // The minify map borrows the pre-minify `chunk.content` (as `sourcesContent`,
      // which the collapse discards), so collapse before swapping in the minified
      // content instead of paying an `into_owned` copy of the whole chunk text.
      let (minified_content, collapsed_map) = {
        // TODO: Do we need to ensure `chunk.preliminary_filename` to be absolute path?
        let (minified_content, new_map, chunk_property_mangle_cache) = EcmaCompiler::dce_or_minify(
          &allocator_guard,
          chunk.content.try_as_inner_str()?,
          source_type,
          chunk.map.is_some(),
          chunk.preliminary_filename.as_str(),
          compress,
          minify_options.clone(),
          codegen_options,
        );
        if let Some(cache) = chunk_property_mangle_cache {
          property_mangle_cache
            .set(cache)
            .expect("property mangling is limited to one JavaScript chunk");
        }
        let collapsed_map = match (&chunk.map, &new_map) {
          (Some(origin_map), Some(new_map)) => Some(collapse_sourcemaps(&[origin_map, new_map])),
          _ => {
            // TODO: Map is dirty. Should we reset the `chunk.map` to `None`?
            None
          }
        };
        (minified_content, collapsed_map)
      };
      chunk.content = minified_content.into();
      if let Some(map) = collapsed_map {
        chunk.map = Some(map);
      }
      Ok(())
    })?;

    Ok(property_mangle_cache.into_inner().map(|cache| {
      cache
        .into_iter()
        .map(|(original, target)| {
          (original.into_string(), target.map(oxc_str::CompactStr::into_string))
        })
        .collect()
    }))
  }

  /// Checks the final output after `generateBundle`.
  ///
  /// Plugins can emit prebuilt chunks after `minify_chunks`, so the early check alone is not
  /// enough.
  pub(crate) fn validate_mangle_properties_output(
    options: &NormalizedBundlerOptions,
    outputs: &[Output],
  ) -> BuildResult<()> {
    let Some((_, minify_options, _)) = chunk_minify_options(&options.minify) else {
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

fn chunk_minify_options(options: &MinifyOptions) -> Option<(bool, &OxcMinifierOptions, bool)> {
  match options {
    MinifyOptions::Disabled => None,
    MinifyOptions::DeadCodeEliminationOnly(options) => Some((false, options, false)),
    MinifyOptions::Enabled(options) => Some((true, &options.options, options.remove_whitespace)),
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
