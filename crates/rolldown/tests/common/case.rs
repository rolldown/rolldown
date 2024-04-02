use std::{borrow::Cow, path::Path};

use super::fixture::Fixture;
use rolldown::RolldownOutput;
use rolldown_common::Output;
use rolldown_error::BuildError;
use rolldown_sourcemap::SourcemapVisualizer;

pub struct Case {
  fixture: Fixture,
  snapshot: String,
}

impl Case {
  pub fn new(path: impl AsRef<Path>) -> Self {
    // Paths could be UNC format in windows, see https://github.com/rust-lang/rust/issues/42869 for more details
    let path = dunce::simplified(path.as_ref());

    Self { fixture: Fixture::new(path.to_path_buf()), snapshot: String::new() }
  }

  pub fn run(self) {
    std::env::set_var("ROLLDOWN_TEST", "1");
    tokio::runtime::Runtime::new().unwrap().block_on(self.run_inner());
  }

  pub async fn run_inner(mut self) {
    let build_output = self.fixture.compile().await;
    match build_output {
      Ok(assets) => {
        assert!(!self.fixture.test_config().expect_error, "expected error, but got success");
        self.render_assets_to_snapshot(assets);
      }
      Err(errs) => {
        assert!(
          self.fixture.test_config().expect_error,
          "expected success, but got errors: {errs:?}"
        );
        self.render_errors_to_snapshot(errs);
      }
    }
    self.make_snapshot();
    self.fixture.exec();
  }

  fn render_assets_to_snapshot(&mut self, outputs: RolldownOutput) {
    let mut assets = outputs.assets;
    let warnings = outputs.warnings;

    if !warnings.is_empty() {
      self.snapshot.push_str("# warnings\n\n");
      let diagnostics = warnings.into_iter().map(|e| (e.code(), e.into_diagnostic()));
      let rendered = diagnostics
        .flat_map(|(code, diagnostic)| {
          [
            Cow::Owned(format!("## {code}\n")),
            "```text".into(),
            Cow::Owned(diagnostic.to_string()),
            "```".into(),
          ]
        })
        .collect::<Vec<_>>()
        .join("\n");
      self.snapshot.push_str(&rendered);
      self.snapshot.push('\n');
    }

    self.snapshot.push_str("# Assets\n\n");
    assets.sort_by_key(|c| c.file_name().to_string());
    let artifacts = assets
      .iter()
      .filter(|asset| !asset.file_name().contains("$runtime$") && matches!(asset, Output::Chunk(_)))
      .flat_map(|asset| {
        [
          Cow::Owned(format!("## {}\n", asset.file_name())),
          "```js".into(),
          Cow::Borrowed(asset.content().trim()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.push_str(&artifacts);

    if self.fixture.test_config().snapshot_output_stats {
      self.render_stats_to_snapshot(&assets);
    }

    if self.fixture.test_config().sourcemap {
      self.render_sourcemap_visualizer_to_snapshot(&assets);
    }
  }

  fn render_stats_to_snapshot(&mut self, assets: &[Output]) {
    self.snapshot.push_str("\n\n## Output Stats\n\n");
    let stats = assets
      .iter()
      .flat_map(|asset| match asset {
        Output::Chunk(chunk) => {
          vec![Cow::Owned(format!(
            "- {}, is_entry {}, is_dynamic_entry {}, exports {:?}",
            chunk.file_name, chunk.is_entry, chunk.is_dynamic_entry, chunk.exports
          ))]
        }
        Output::Asset(_) => vec![],
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.push_str(&stats);
  }

  fn render_errors_to_snapshot(&mut self, mut errors: Vec<BuildError>) {
    self.snapshot.push_str("# Errors\n\n");
    errors.sort_by_key(BuildError::code);
    let diagnostics = errors.into_iter().map(|e| (e.code(), e.into_diagnostic()));

    let rendered = diagnostics
      .flat_map(|(code, diagnostic)| {
        [
          Cow::Owned(format!("## {code}\n")),
          "```text".into(),
          Cow::Owned(diagnostic.to_string()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.push_str(&rendered);
  }

  fn render_sourcemap_visualizer_to_snapshot(&mut self, assets: &[Output]) {
    self.snapshot.push_str("\n\n# Sourcemap Visualizer\n\n");
    let visualizer_result = assets
      .iter()
      .filter(|asset| !asset.file_name().contains("$runtime$"))
      .filter_map(|asset| match asset {
        Output::Chunk(chunk) => chunk
          .map
          .as_ref()
          .map(|sourcemap| SourcemapVisualizer::new(&chunk.code, sourcemap).into_visualizer_text()),
        Output::Asset(_) => None,
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.push_str(&visualizer_result);
  }

  fn make_snapshot(&self) {
    // Configure insta to use the fixture path as the snapshot path
    let fixture_folder = self.fixture.dir_path();
    let mut settings = insta::Settings::clone_current();
    let content = self.snapshot.to_string();
    settings.set_snapshot_path(fixture_folder);
    settings.set_prepend_module_to_snapshot(false);
    settings.set_input_file(fixture_folder);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", content);
    });
  }
}
