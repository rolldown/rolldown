use std::path::PathBuf;

use rolldown::BundleOutput;
use rolldown_error::BuildResult;

/// After support config variants, a test case might run the bundler multiple times with different configs.
/// For each run, we call it a "build round".
/// This struct contains all the information we want to snapshot for a normal build round.
#[derive(Default)]
pub struct BuildRoundOutput {
  pub overwritten_test_meta_snapshot: bool,
  pub cwd: Option<PathBuf>,
  pub debug_title: Option<String>,
  pub initial_output: Option<BuildResult<BundleOutput>>,
}
