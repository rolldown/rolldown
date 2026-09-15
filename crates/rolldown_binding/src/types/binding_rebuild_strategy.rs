use napi_derive::napi;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[napi]
pub enum BindingRebuildStrategy {
  Always,
  Never,
}

impl From<BindingRebuildStrategy> for rolldown_dev::RebuildStrategy {
  fn from(value: BindingRebuildStrategy) -> Self {
    match value {
      BindingRebuildStrategy::Always => Self::Always,
      BindingRebuildStrategy::Never => Self::Never,
    }
  }
}
