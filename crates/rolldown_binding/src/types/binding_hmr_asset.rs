use std::sync::Arc;

use napi::Either;
use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

/// An asset emitted while computing an HMR patch (see `HmrPatch::assets`). An HMR
/// update runs no `generate`, so the consumer must register these for the asset
/// URLs the patch references to resolve on the first request.
/// See `meta/design/plugin-asset-module.md` (rolldown#9812 / vitejs/vite#22596).
#[napi(object)]
pub struct BindingHmrAsset {
  pub filename: String,
  #[napi(ts_type = "string | Uint8Array")]
  pub source: Either<String, Buffer>,
}

impl std::fmt::Debug for BindingHmrAsset {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingHmrAsset")
      .field("filename", &self.filename)
      .field(
        "source",
        match &self.source {
          Either::A(s) => s,
          Either::B(_) => &"<buffer>",
        },
      )
      .finish()
  }
}

impl From<Arc<rolldown_common::OutputAsset>> for BindingHmrAsset {
  fn from(asset: Arc<rolldown_common::OutputAsset>) -> Self {
    let source = match &asset.source {
      rolldown_common::StrOrBytes::Str(value) => Either::A(value.clone()),
      rolldown_common::StrOrBytes::Bytes(value) => Either::B(value.clone().into()),
    };
    Self { filename: asset.filename.to_string(), source }
  }
}
