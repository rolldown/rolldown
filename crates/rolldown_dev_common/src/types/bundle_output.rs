use std::collections::BTreeMap;

use rolldown_common::Output;
use rolldown_error::BuildDiagnostic;

#[derive(Default)]
pub struct BundleOutput {
  pub warnings: Vec<BuildDiagnostic>,
  pub assets: Vec<Output>,
  pub mangle_cache: Option<BTreeMap<String, Option<String>>>,
  /// Dev mode only: the HMR generation this output is rendered at. Its code does not carry it,
  /// so the consumer passes it to the page that loads this output.
  pub hmr_generation: Option<u32>,
}
