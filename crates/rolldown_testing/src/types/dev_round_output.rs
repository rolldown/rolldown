use std::path::PathBuf;

use rolldown::BundleOutput;
use rolldown_error::BuildResult;

/// After support config variants, a test case might run the bundler multiple times with different configs.
/// For each run, we call it a "build round" or "dev round".
/// For HMR/dev mode, a dev round contains the initial build plus rebuilds triggered by file changes.
/// This struct contains all the information we want to snapshot for a dev round with HMR.
#[derive(Default)]
pub struct DevRoundOutput {
  pub overwritten_test_meta_snapshot: bool,
  pub cwd: Option<PathBuf>,
  pub debug_title: Option<String>,
  pub initial_output: Option<BuildResult<BundleOutput>>,
  pub rebuild_results: Vec<BuildResult<BundleOutput>>,
  pub hmr_updates_by_steps: Vec<BuildResult<(Vec<rolldown_common::ClientHmrUpdate>, Vec<String>)>>,
}
