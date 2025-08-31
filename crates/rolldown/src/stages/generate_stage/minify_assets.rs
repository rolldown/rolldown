use oxc::{
  codegen::{self, CodegenOptions, CommentOptions},
  minifier::{CompressOptions, MinifierOptions},
};
use oxc_allocator::AllocatorPool;
use rolldown_common::{LegalComments, MinifyOptions, NormalizedBundlerOptions};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::BuildResult;
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::AssetVec;

use super::GenerateStage;

impl GenerateStage<'_> {
  pub fn minify_assets(
    options: &NormalizedBundlerOptions,
    assets: &mut AssetVec,
  ) -> BuildResult<()> {
    let (compress, minify_option) = match &options.minify {
      MinifyOptions::Disabled => return Ok(()),
      MinifyOptions::DeadCodeEliminationOnly => {
        (false, &MinifierOptions { mangle: None, compress: Some(CompressOptions::default()) })
      }
      MinifyOptions::Enabled(options) => (true, options),
    };
    let remove_whitespace = compress;
    let allocator_pool = AllocatorPool::new(rayon::current_num_threads());
    assets.par_iter_mut().try_for_each(|asset| -> anyhow::Result<()> {
      if test_d_ts_pattern(&asset.filename) {
        return Ok(());
      }
      match asset.meta {
        rolldown_common::InstantiationKind::Ecma(_) => {
          let codegen_options = CodegenOptions {
            minify: remove_whitespace,
            comments: CommentOptions {
              normal: false,
              jsdoc: false,
              annotation: !remove_whitespace,
              legal: if matches!(options.legal_comments, LegalComments::Inline)
                || !remove_whitespace
              {
                codegen::LegalComment::Inline
              } else {
                codegen::LegalComment::None
              },
            },
            ..CodegenOptions::default()
          };

          let allocator_guard = allocator_pool.get();
          // TODO: Do we need to ensure `asset.filename` to be absolute path?
          let (minified_content, new_map) = EcmaCompiler::dce_or_minify(
            &allocator_guard,
            asset.content.try_as_inner_str()?,
            options.format.source_type().with_jsx(true),
            asset.map.is_some(),
            &asset.filename,
            compress,
            minify_option.clone(),
            codegen_options,
          );
          asset.content = minified_content.into();
          match (&asset.map, &new_map) {
            (Some(origin_map), Some(new_map)) => {
              asset.map = Some(collapse_sourcemaps(&[origin_map, new_map]));
            }
            _ => {
              // TODO: Map is dirty. Should we reset the `asset.map` to `None`?
            }
          }
        }
        rolldown_common::InstantiationKind::Css(_)
        | rolldown_common::InstantiationKind::None
        | rolldown_common::InstantiationKind::Sourcemap(_) => {}
      }
      Ok(())
    })?;

    Ok(())
  }
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
