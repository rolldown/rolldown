use std::collections::BTreeMap;

use rolldown_common::Output;
use rolldown_error::BuildDiagnostic;

#[derive(Default)]
pub struct BundleOutput {
  pub warnings: Vec<BuildDiagnostic>,
  pub assets: Vec<Output>,
  pub mangle_cache: Option<BTreeMap<String, Option<String>>>,
}
