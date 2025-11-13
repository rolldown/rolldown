use crate::dev::dev_context::BundlingFuture;

/// Return value for `ensure_latest_bundle_output` containing the bundling future to await
#[derive(Debug, Clone)]
pub struct EnsureLatestBundleOutputReturn {
  /// The bundling task future to wait for
  pub future: BundlingFuture,
  pub is_ensure_latest_bundle_output_future: bool,
}
