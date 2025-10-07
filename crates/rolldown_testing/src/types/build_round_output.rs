use std::path::PathBuf;

use rolldown::BundleOutput;
use rolldown_error::BuildResult;

/// After support config variants, a test case might run the bundler multiple times with different configs.
/// For each run, we call it a "build round".
/// After support testing HMR, a build round might contain multiple builds (initial build + rebuilds).
/// This struct contains all the information we want to snapshot for a build round.
#[derive(Default)]
pub struct BuildRoundOutput {
  pub overwritten_test_meta_snapshot: bool,
  pub cwd: Option<PathBuf>,
  pub debug_title: Option<String>,
  pub initial_output: Option<BuildResult<BundleOutput>>,
  pub rebuild_results: Vec<BuildResult<BundleOutput>>,
  pub hmr_updates_by_steps: Vec<BuildResult<(Vec<rolldown_common::ClientHmrUpdate>, Vec<String>)>>,
}
