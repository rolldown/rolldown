use rolldown::BundleOutput;
use rolldown_error::BuildResult;

use crate::hmr_test_client::HmrClientStepRecord;

/// Represents the outputs from a single HMR step.
/// A step consists of file changes that trigger HMR updates and potentially build outputs.
pub struct HmrStepOutput {
  /// The HMR update generated for this step (one for every client), with the changed files.
  /// `None` when the step sent no update.
  pub hmr_update: BuildResult<(Option<rolldown_common::HmrUpdate>, Vec<String>)>,
  /// What the test client did with the push: its patch requests and the result. `None` when
  /// no client ran the push.
  pub client: Option<HmrClientStepRecord>,
  /// Build outputs triggered by this HMR step.
  /// Can be empty (pure HMR patch with no rebuild),
  /// one (a rebuild forced by the rebuild strategy or an explicit trigger),
  /// or multiple (if multiple rebuilds are triggered).
  pub build_outputs: Vec<BuildResult<BundleOutput>>,
}
