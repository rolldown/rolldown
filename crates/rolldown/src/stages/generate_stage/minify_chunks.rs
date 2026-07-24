use oxc::{
  codegen::{self, CodegenOptions, CommentOptions},
  minifier::ManglePropertyCache,
};
use oxc_allocator::AllocatorPool;
use oxc_str::CompactStr;
use rolldown_common::{MinifyOptions, NormalizedBundlerOptions};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use crate::type_alias::IndexInstantiatedChunks;

use super::GenerateStage;

impl GenerateStage<'_> {
  #[tracing::instrument(level = "debug", skip_all)]
  pub fn minify_chunks(
    options: &NormalizedBundlerOptions,
    chunks: &mut IndexInstantiatedChunks,
  ) -> BuildResult<Vec<BuildDiagnostic>> {
    let (compress, minify_option, remove_whitespace) = match &options.minify {
      MinifyOptions::Disabled => return Ok(Vec::new()),
      MinifyOptions::DeadCodeEliminationOnly(options) => (false, options, false),
      MinifyOptions::Enabled((options, remove_whitespace)) => (true, options, *remove_whitespace),
    };
    let allocator_pool = AllocatorPool::new(rayon::current_num_threads());
    let property_mangle_caches = chunks
      .par_iter_mut()
      .map(|chunk| -> anyhow::Result<Option<(String, ManglePropertyCache)>> {
        if test_d_ts_pattern(chunk.preliminary_filename.as_str()) {
          return Ok(None);
        }
        let property_mangle_cache = match chunk.kind {
          rolldown_common::InstantiationKind::Ecma(_) => {
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
            let (minified_content, collapsed_map, property_mangle_cache) = {
              // TODO: Do we need to ensure `chunk.preliminary_filename` to be absolute path?
              let (minified_content, new_map, property_mangle_cache) = EcmaCompiler::dce_or_minify(
                &allocator_guard,
                chunk.content.try_as_inner_str()?,
                options.format.source_type().with_jsx(true),
                chunk.map.is_some(),
                chunk.preliminary_filename.as_str(),
                compress,
                minify_option.clone(),
                codegen_options,
              );
              let collapsed_map = match (&chunk.map, &new_map) {
                (Some(origin_map), Some(new_map)) => {
                  Some(collapse_sourcemaps(&[origin_map, new_map]))
                }
                _ => {
                  // TODO: Map is dirty. Should we reset the `chunk.map` to `None`?
                  None
                }
              };
              (minified_content, collapsed_map, property_mangle_cache)
            };
            chunk.content = minified_content.into();
            if let Some(map) = collapsed_map {
              chunk.map = Some(map);
            }
            property_mangle_cache
          }
          rolldown_common::InstantiationKind::None
          | rolldown_common::InstantiationKind::Sourcemap(_) => None,
        };
        Ok(property_mangle_cache.map(|cache| (chunk.preliminary_filename.to_string(), cache)))
      })
      .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(cross_chunk_property_mangle_warning(property_mangle_caches).into_iter().collect())
  }
}

fn cross_chunk_property_mangle_warning(
  caches: Vec<Option<(String, ManglePropertyCache)>>,
) -> Option<BuildDiagnostic> {
  let mut filenames = Vec::new();
  let mut mappings: FxHashMap<CompactStr, Vec<(usize, Option<CompactStr>)>> = FxHashMap::default();
  for (filename, cache) in caches.into_iter().flatten() {
    let chunk_index = filenames.len();
    filenames.push(filename);
    for (original, target) in cache {
      mappings.entry(original).or_default().push((chunk_index, target));
    }
  }

  let mut conflicts = mappings
    .into_iter()
    .filter_map(|(original, mut chunk_mappings)| {
      let first_target = chunk_mappings.first()?.1.as_ref();
      if chunk_mappings.iter().all(|(_, target)| target.as_ref() == first_target) {
        return None;
      }
      chunk_mappings.sort_unstable_by_key(|mapping| mapping.0);
      Some((
        original.to_string(),
        chunk_mappings
          .into_iter()
          .map(|(chunk_index, target)| {
            (filenames[chunk_index].clone(), target.map(|target| target.to_string()))
          })
          .collect(),
      ))
    })
    .collect::<Vec<_>>();
  conflicts.sort_unstable_by(|a, b| a.0.cmp(&b.0));

  (!conflicts.is_empty())
    .then(|| BuildDiagnostic::cross_chunk_property_mangle(conflicts).with_severity_warning())
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
