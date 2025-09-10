use itertools::Either;
#[derive(Debug)]
pub enum ScanMode<T> {
  Full,
  //// vector of module id
  Partial(Vec<T>),
}

impl<T> ScanMode<T> {
  /// Returns `true` if the scan mode is [`Full`].
  ///
  /// [`Full`]: ScanMode::Full
  #[must_use]
  #[inline]
  pub fn is_full(&self) -> bool {
    matches!(self, Self::Full)
  }

  pub fn iter(&self) -> impl Iterator<Item = &T> {
    match self {
      ScanMode::Full => Either::Left(std::iter::empty()),
      ScanMode::Partial(ids) => Either::Right(ids.iter()),
    }
  }
}
