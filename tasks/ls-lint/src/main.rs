use std::collections::HashMap;

use anyhow::Result;
use globset::{Glob, GlobMatcher, GlobSetBuilder};
use heck::ToKebabCase;
use ignore::WalkBuilder;
use rolldown_testing::workspace::root_dir;
use serde::Deserialize;

#[derive(Deserialize, Debug, Copy, Clone)]
enum Case {
  #[serde(rename(deserialize = "kebab-case"))]
  KebabCase,
}

#[allow(clippy::zero_sized_map_values)]
#[derive(Deserialize, Debug)]
struct LsLintConfig {
  ignore: Vec<String>,
  ls: HashMap<String, Case>,
}

fn main() -> anyhow::Result<()> {
  let config = load_config()?;
  let builder = WalkBuilder::new(root_dir());

  let mut globset_builder = GlobSetBuilder::new();
  for ignore in &config.ignore {
    globset_builder.add(Glob::new(ignore)?);
  }
  let globset_filter = globset_builder.build()?;
  let ls_map = config
    .ls
    .iter()
    .map(|(k, v)| {
      let glob = Glob::new(k).unwrap().compile_matcher();
      (glob, *v)
    })
    .collect::<Vec<(GlobMatcher, Case)>>();

  let walk = builder.build();
  let mut has_error = false;
  for result in walk {
    let result = result?;
    let relative_path = result.path().strip_prefix(root_dir())?;

    if globset_filter.is_match(relative_path) {
      continue;
    }

    let file_name = result.file_name().to_string_lossy();
    for (pattern, case) in &ls_map {
      if pattern.is_match(file_name.as_ref()) {
        let file_stem = file_name.split('.').next().unwrap_or(&file_name);
        match case {
          Case::KebabCase if file_stem != file_stem.to_kebab_case() => {
            has_error = true;
            eprintln!("{} should be kebab-case", relative_path.display());
          }
          Case::KebabCase => {}
        };
      }
    }
  }
  assert!(!has_error, "ls-lint failed");

  Ok(())
}

fn load_config() -> Result<LsLintConfig> {
  let source = include_str!("../../../.ls-lint.json");
  let config = serde_json::from_str::<LsLintConfig>(source)?;
  Ok(config)
}
