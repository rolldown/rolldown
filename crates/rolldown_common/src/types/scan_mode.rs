use arcstr::ArcStr;
#[derive(Debug)]
pub enum ScanMode {
  Full,
  //// vector of module id
  Partial(Vec<ArcStr>),
}

impl ScanMode {
  /// Returns `true` if the scan mode is [`Full`].
  ///
  /// [`Full`]: ScanMode::Full
  #[must_use]
  pub fn is_full(&self) -> bool {
    matches!(self, Self::Full)
  }
}
