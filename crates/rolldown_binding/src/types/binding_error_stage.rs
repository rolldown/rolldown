use napi_derive::napi;
use rolldown_dev::ErrorStage;

/// Which stage of an incremental dev build produced the last error.
///
/// Mirrors `rolldown_dev::ErrorStage`. Surfaced on
/// [`crate::binding_dev_engine::BindingBundleState`] so the consumer can
/// treat an `Hmr`-stage failure as recoverable by forcing a full rebuild
/// on the next page load (HMR generation may itself be buggy). See
/// `meta/design/dev-engine.md` §12.
#[derive(Debug, Clone, Copy)]
#[napi(string_enum)]
pub enum BindingErrorStage {
  Hmr,
  Rebuild,
}

impl From<ErrorStage> for BindingErrorStage {
  fn from(value: ErrorStage) -> Self {
    match value {
      ErrorStage::Hmr => Self::Hmr,
      ErrorStage::Rebuild => Self::Rebuild,
    }
  }
}
