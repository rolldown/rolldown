use std::{borrow::Cow, path::Path};

use rolldown::Asset;
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
    tokio::runtime::Runtime::new().unwrap().block_on(self.run_inner())
  }

  pub async fn run_inner(mut self) {
    let assets = self.fixture.compile().await;
    let snapshot = self.convert_assets_to_snapshot(assets);
    self.make_snapshot();
    self.fixture.exec();
  }

  fn convert_assets_to_snapshot(&mut self, mut assets: Vec<Asset>) {
    self.snapshot.append("# Artifacts\n\n");
    assets.sort_by_key(|c| c.file_name.clone());
    let artifacts = assets
      .iter()
      // FIXME: should render the runtime module while tree shaking being supported
      .filter(|asset| !asset.file_name.contains("rolldown_runtime"))
      .flat_map(|asset| {
        [
          Cow::Owned(format!("## {}\n", asset.file_name)),
          "```js".into(),
          Cow::Borrowed(asset.content.trim()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n");
    self.snapshot.append(artifacts);
  }

  fn make_snapshot(&self) {
    // Configure insta to use the fixture path as the snapshot path
    let fixture_folder = self.fixture.dir_path();
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(fixture_folder);
    settings.set_prepend_module_to_snapshot(false);
    settings.set_input_file(fixture_folder);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", self.snapshot.to_string());
    });
  }
}
