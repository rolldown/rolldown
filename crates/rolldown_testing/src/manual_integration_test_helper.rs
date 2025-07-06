use std::path::PathBuf;

use rolldown_testing_config::TestMeta;

use crate::integration_test::IntegrationTest;

/// This struct is to help manually writing `IntegrationTest` in rust.
/// Do not use this struct directly, use the macro `integration_test_builder!` instead.
#[derive(Default)]
pub struct IntegrationTestBuilder {
  // Absolute path of the test folder. It may or may not contain the `_config.json` file.
  test_folder_path: PathBuf,
}

impl IntegrationTestBuilder {
  pub fn new(test_folder_path: PathBuf) -> Self {
    Self { test_folder_path }
  }

  pub fn build(self, meta: TestMeta) -> IntegrationTest {
    let test_folder_path = self.test_folder_path;

    IntegrationTest::new(meta, test_folder_path)
  }
}

#[macro_export]
macro_rules! manual_integration_test {
  () => {
    $crate::manual_integration_test_helper::IntegrationTestBuilder::new($crate::abs_file_dir!())
  };
}
