use std::{borrow::Cow, path::Path};

use rolldown::Output;
use rolldown_error::BuildError;
use string_wizard::MagicString;

use super::fixture::Fixture;

pub struct Case {
  fixture: Fixture,
  snapshot: MagicString<'static>,
}

impl Case {
  pub fn new(path: impl AsRef<Path>) -> Self {
    Self { fixture: Fixture::new(path.as_ref().to_path_buf()), snapshot: MagicString::new("") }
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

  fn render_assets_to_snapshot(&mut self, mut assets: Vec<Output>) {
    self.snapshot.append("# Assets\n\n");
    assets.sort_by_key(|c| c.file_name().to_string());
    let artifacts = assets
      .iter()
      // FIXME: should render the runtime module while tree shaking being supported
      .filter(|asset| !asset.file_name().contains("rolldown_runtime"))
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
    self.snapshot.append(artifacts);
  }

  fn render_errors_to_snapshot(&mut self, mut errors: Vec<BuildError>) {
    self.snapshot.append("# Errors\n\n");
    errors.sort_by_key(rolldown_error::BuildError::code);
    let rendered = errors
      .iter()
      .flat_map(|error| {
        [
          Cow::Owned(format!("## {}\n", error.code())),
          "```text".into(),
          Cow::Owned(error.to_diagnostic().print_to_string()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.append(rendered);
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
