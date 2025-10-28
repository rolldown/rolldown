use rolldown::BundleOutput;
use rolldown_error::BuildResult;

/// Represents the outputs from a single HMR step.
/// A step consists of file changes that trigger HMR updates and potentially build outputs.
pub struct HmrStepOutput {
  /// The HMR updates generated for this step.
  /// Contains updates for all clients (Vec<ClientHmrUpdate>) and the changed files.
  pub hmr_updates: BuildResult<(Vec<rolldown_common::ClientHmrUpdate>, Vec<String>)>,
  /// Build outputs triggered by this HMR step.
  /// Can be empty (pure HMR patch with no rebuild),
  /// one (typical full-reload scenario),
  /// or multiple (if multiple rebuilds are triggered).
  pub build_outputs: Vec<BuildResult<BundleOutput>>,
}
