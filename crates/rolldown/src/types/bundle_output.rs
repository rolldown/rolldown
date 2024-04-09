use rolldown_common::Output;
use rolldown_error::BuildError;

#[derive(Debug)]
pub struct BundleOutput {
  pub warnings: Vec<BuildError>,
  pub assets: Vec<Output>,
}
