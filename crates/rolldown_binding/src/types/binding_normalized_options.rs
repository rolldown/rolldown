use std::collections::HashMap;

use napi::Either;
use napi_derive::napi;
use rolldown::SharedNormalizedBundlerOptions;

#[napi]
pub struct BindingNormalizedOptions {
  inner: SharedNormalizedBundlerOptions,
}

#[napi]
impl BindingNormalizedOptions {
  pub fn new(inner: SharedNormalizedBundlerOptions) -> Self {
    Self { inner }
  }

  #[napi(getter)]
  pub fn shim_missing_exports(&self) -> bool {
    self.inner.shim_missing_exports
  }

  // Notice: rust's HashMap doesn't guarantee the order of keys, so not sure if it's a good idea to expose it to JS directly.
  #[napi(getter)]
  pub fn input(&self) -> Either<Vec<String>, HashMap<String, String>> {
    let mut inputs_iter = self.inner.input.iter().peekable();
    let has_name = inputs_iter.peek().is_some_and(|input| input.name.is_some());
    if has_name {
      Either::B(
        self
          .inner
          .input
          .iter()
          .map(|input| {
            (
              input.name.clone().unwrap_or_else(|| {
                unreachable!("Inputs passed from js side are either all have names or not")
              }),
              input.import.clone(),
            )
          })
          .collect(),
      )
    } else {
      Either::A(self.inner.input.iter().map(|input| input.import.clone()).collect())
    }
  }

  // For `Fn` variant, we can't convert it to JS, so we just return `None`.
  #[napi(getter)]
  pub fn entry_filenames(&self) -> Option<String> {
    match &self.inner.entry_filenames {
      rolldown::ChunkFilenamesOutputOption::String(inner) => Some(inner.clone()),
      rolldown::ChunkFilenamesOutputOption::Fn(_) => None,
    }
  }
}
