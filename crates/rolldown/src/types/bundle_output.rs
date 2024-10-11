use arcstr::ArcStr;
use rolldown_common::Output;
use rolldown_error::BuildDiagnostic;

#[derive(Default)]
pub struct BundleOutput {
  pub warnings: Vec<BuildDiagnostic>,
  pub errors: Vec<BuildDiagnostic>,
  pub assets: Vec<Output>,
  pub watch_files: Vec<ArcStr>,
}
