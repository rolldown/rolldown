use rolldown_common::Output;
use rolldown_error::BuildError;

#[derive(Default)]
pub struct BundleOutput {
  pub warnings: Vec<BuildError>,
  pub errors: Vec<BuildError>,
  pub assets: Vec<Output>,
}
