mod common;

use std::path::PathBuf;

use common::Case;
use testing_macros::fixture;

#[fixture("./tests/esbuild/**/test.config.json")]
fn test(path: PathBuf) {
  Case::new(path.parent().unwrap()).exec();
}
