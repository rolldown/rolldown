use std::collections::HashMap;

use anyhow::Result;
use globset::{Glob, GlobMatcher, GlobSetBuilder};
use heck::{ToKebabCase, ToSnakeCase};
use ignore::WalkBuilder;
use rolldown_workspace::root_dir;
use serde::Deserialize;

#[derive(Deserialize, Debug, Copy, Clone)]
enum Case {
  #[serde(rename(deserialize = "kebab-case"))]
  KebabCase,
  #[serde(rename(deserialize = "snake_case"))]
  SnakeCase,
}

#[allow(clippy::zero_sized_map_values)]
#[derive(Deserialize, Debug)]
struct LsLintConfig {
  ignore: Vec<String>,
  file: HashMap<String, Case>,
  directory: HashMap<String, Case>,
}

fn main() -> anyhow::Result<()> {
  let config = load_config()?;
  let builder = WalkBuilder::new(root_dir());

  let mut globset_builder = GlobSetBuilder::new();
  for ignore in &config.ignore {
    globset_builder.add(Glob::new(ignore)?);
  }
  let globset_filter = globset_builder.build()?;
  let file_matcher_map = config
    .file
    .iter()
    .map(|(k, v)| {
      let glob = Glob::new(k).unwrap().compile_matcher();
      (glob, *v)
    })
    .collect::<Vec<(GlobMatcher, Case)>>();

  let directory_matcher_map = config
    .directory
    .iter()
    .map(|(k, v)| {
      let glob = Glob::new(k).unwrap().compile_matcher();
      (glob, *v)
    })
    .collect::<Vec<(GlobMatcher, Case)>>();

  let walk = builder.build();
  let mut error_count = 0;
  for result in walk {
    let result = result?;
    let relative_path = result.path().strip_prefix(root_dir())?;

    if globset_filter.is_match(relative_path) {
      continue;
    }
    if relative_path.is_dir() {
      for (pattern, case) in &directory_matcher_map {
        if pattern.is_match(relative_path) {
          let base_name = relative_path.file_name().unwrap().to_string_lossy();
          match case {
            Case::KebabCase if base_name != base_name.to_kebab_case() => {
              error_count += 1;
              eprintln!("{} should be kebab-case", relative_path.display());
            }
            Case::SnakeCase if base_name != base_name.to_snake_case() => {
              error_count += 1;
              eprintln!("{} should be snake_case", relative_path.display());
            }
            _ => {}
          }
        }
      }
    } else {
      let file_name = result.file_name().to_string_lossy();
      for (pattern, case) in &file_matcher_map {
        if pattern.is_match(file_name.as_ref()) {
          let file_stem = file_name.split('.').next().unwrap_or(&file_name);
          match case {
            Case::KebabCase if file_stem != file_stem.to_kebab_case() => {
              error_count += 1;
              eprintln!("{} should be kebab-case", relative_path.display());
            }
            Case::SnakeCase if file_stem != file_stem.to_snake_case() => {
              error_count += 1;
              eprintln!("{} should be snake_case", relative_path.display());
            }
            _ => {}
          }
        }
      }
    }
  }
  assert!(error_count == 0, "ls-lint failed with {error_count} errors");

  Ok(())
}

fn load_config() -> Result<LsLintConfig> {
  let source = include_str!("../../../.ls-lint.json");
  let config = serde_json::from_str::<LsLintConfig>(source)?;
  Ok(config)
}
