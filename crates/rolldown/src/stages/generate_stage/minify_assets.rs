use oxc::codegen::CodegenOptions;
use rolldown_common::{LegalComments, MinifyOptions};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_error::BuildResult;
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_utils::rayon::{IntoParallelRefMutIterator, ParallelIterator};

use crate::type_alias::IndexAssets;

use super::GenerateStage;

impl GenerateStage<'_> {
  pub fn minify_assets(&self, assets: &mut IndexAssets) -> BuildResult<()> {
    if let MinifyOptions::Enabled(minify_options) = &self.options.minify {
      assets.par_iter_mut().try_for_each(|asset| -> anyhow::Result<()> {
        if test_d_ts_pattern(&asset.filename) {
          return Ok(());
        }
        match asset.meta {
          rolldown_common::InstantiationKind::Ecma(_) => {
            // TODO: Do we need to ensure `asset.filename` to be absolute path?
            let (minified_content, new_map) = EcmaCompiler::minify(
              asset.content.try_as_inner_str()?,
              asset.map.is_some(),
              &asset.filename,
              minify_options.to_oxc_minifier_options(self.options),
              minify_options.compress,
              if minify_options.remove_whitespace {
                CodegenOptions::minify()
              } else {
                CodegenOptions { comments: false, ..CodegenOptions::default() }
              },
              matches!(self.options.legal_comments, LegalComments::Inline),
            );
            asset.content = minified_content.into();
            match (&asset.map, &new_map) {
              (Some(origin_map), Some(new_map)) => {
                asset.map = Some(collapse_sourcemaps(vec![origin_map, new_map]));
              }
              _ => {
                // TODO: Map is dirty. Should we reset the `asset.map` to `None`?
              }
            }
          }
          rolldown_common::InstantiationKind::Css(_) | rolldown_common::InstantiationKind::None => {
          }
        }
        Ok(())
      })?;
    }

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
