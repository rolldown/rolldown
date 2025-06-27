#![allow(clippy::ignore_without_reason)]
use std::fmt::Write as _;
use std::path::{Component, PathBuf};

use rolldown_common::Output;
use rolldown_testing::{
  fixture::Fixture,
  integration_test::IntegrationTest,
  test_config::{TestConfig, TestMeta, read_test_config},
};
use sugar_path::SugarPath;
use testing_macros::fixture;

mod rolldown;

#[allow(clippy::needless_pass_by_value)]
#[fixture("./tests/rolldown/**/_config.json")]
fn fixture_with_config(config_path: PathBuf) {
  Fixture::new(config_path.parent().unwrap()).run_integration_test();
}

#[tokio::test(flavor = "multi_thread")]
async fn filename_with_hash() {
  let mut snapshot_outputs = vec![];

  let mut config_paths =
    glob::glob("./tests/**/_config.json").unwrap().map(Result::unwrap).collect::<Vec<_>>();
  let cwd = std::env::current_dir().unwrap();
  config_paths.sort_by_cached_key(|p| p.relative(&cwd));

  for path in config_paths {
    if path.components().map(Component::as_os_str).any(|c| c.to_string_lossy().starts_with('.')) {
      continue;
    }
    let mut snapshot_output = String::new();
    let config_path = path.canonicalize().unwrap();
    let config_path = dunce::simplified(&config_path);
    let fixture_path = config_path.parent().unwrap();

    let TestConfig { config: mut options, meta, config_variants: _not_supported } =
      read_test_config(config_path);

    if options.cwd.is_none() {
      options.cwd = Some(fixture_path.to_path_buf());
    }

    options.canonicalize_option_path();

    let integration_test =
      IntegrationTest::new(TestMeta { write_to_disk: false, hash_in_filename: true, ..meta });
    if meta.expect_error {
      // Output is expected to be dirty. No need to record.
      continue;
    }
    let assets = integration_test.bundle(options).await.unwrap();

    write!(snapshot_output, "# {}\n\n", fixture_path.relative(&cwd).to_slash_lossy()).unwrap();

    assets.assets.iter().for_each(|asset| match asset {
      Output::Asset(asset) => {
        writeln!(snapshot_output, "- {}", asset.filename).unwrap();
      }
      Output::Chunk(chunk) => {
        writeln!(
          snapshot_output,
          "- {} => {}",
          chunk.preliminary_filename.as_str(),
          chunk.filename.as_str()
        )
        .unwrap();
      }
    });

    snapshot_outputs.push(snapshot_output);
  }
  let output = format!("```\n{}\n```", snapshot_outputs.join("\n"));
  insta::assert_snapshot!(output);
}
