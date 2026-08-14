#[derive(Debug, Clone, Copy)]
#[expect(clippy::enum_variant_names)]
pub enum BundleMode {
  // Normal build
  FullBuild,
  // A full build with enabling `incremental` option.
  // Compared to `FullBuild`, `IncrementalFullBuild` needss to store the data into cache for future incremental builds, while `FullBuild` does not care about that.
  IncrementalFullBuild,
  // An incremental build only for changed files
  IncrementalBuild,
}

impl BundleMode {
  pub fn is_full_build(&self) -> bool {
    matches!(self, BundleMode::FullBuild | BundleMode::IncrementalFullBuild)
  }

  pub fn is_incremental(&self) -> bool {
    matches!(self, BundleMode::IncrementalFullBuild | BundleMode::IncrementalBuild)
  }
}
