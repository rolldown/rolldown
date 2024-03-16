use rolldown_common::Output;
use rolldown_error::BuildError;

pub struct RolldownOutput {
  pub warnings: Vec<BuildError>,
  pub assets: Vec<Output>,
}
