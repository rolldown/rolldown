use std::{borrow::Cow, path::Path};

use rolldown::Asset;

use super::fixture::Fixture;

pub struct Case {
  fixture: Fixture,
}

impl Case {
  pub fn new(path: impl AsRef<Path>) -> Self {
    Self { fixture: Fixture::new(path.as_ref().to_path_buf()) }
  }

  pub fn exec(self) {
    tokio::runtime::Runtime::new().unwrap().block_on(self.exec_inner())
  }

  pub async fn exec_inner(mut self) {
    let assets = self.fixture.compile().await;
    let snapshot = Self::convert_assets_to_snapshot(assets);
    self.take_snapshot(&snapshot);
  }

  fn convert_assets_to_snapshot(mut assets: Vec<Asset>) -> String {
    assets.sort_by_key(|c| c.file_name.clone());
    assets
      .iter()
      // FIXME: should render the runtime module while tree shaking being supported
      .filter(|asset| !asset.file_name.contains("rolldown_runtime"))
      .flat_map(|asset| {
        [
          Cow::Owned(format!("# {}\n", asset.file_name)),
          "```js".into(),
          Cow::Borrowed(asset.content.trim()),
          "```".into(),
        ]
      })
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn take_snapshot(&self, content: &str) {
    // Configure insta to use the fixture path as the snapshot path
    let fixture_folder = self.fixture.dir_path();
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_path(fixture_folder);
    settings.set_prepend_module_to_snapshot(false);
    settings.set_input_file(fixture_folder);
    settings.bind(|| {
      insta::assert_snapshot!("artifacts", content);
    });
  }
}
