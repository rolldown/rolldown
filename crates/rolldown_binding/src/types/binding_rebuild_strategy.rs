use napi_derive::napi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi]
pub enum BindingRebuildStrategy {
  Always,
  Auto,
  Never,
}

impl From<BindingRebuildStrategy> for rolldown::dev::RebuildStrategy {
  fn from(value: BindingRebuildStrategy) -> Self {
    match value {
      BindingRebuildStrategy::Always => Self::Always,
      BindingRebuildStrategy::Auto => Self::Auto,
      BindingRebuildStrategy::Never => Self::Never,
    }
  }
}
