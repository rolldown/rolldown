use napi::Either;
use napi::bindgen_prelude::Uint8Array;

// This struct is used to both pass to JS and receive from JS:
// - Pass to JS: `From<StrOrBytes>` impl (line 44) in BindingOutputAsset.source getter
// - Receive from JS: `From<BindingAssetSource>` impl (line 35) in BindingEmittedAsset
#[napi_derive::napi(object)]
pub struct BindingAssetSource {
  #[napi(ts_type = "string | Uint8Array")]
  pub inner: Either<String, Uint8Array>,
}

#[inline]
fn default_source() -> Either<String, Uint8Array> {
  Either::A(String::default())
}

impl Default for BindingAssetSource {
  fn default() -> Self {
    Self { inner: default_source() }
  }
}

impl std::fmt::Debug for BindingAssetSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingAssetSource")
      .field(
        "inner",
        match &self.inner {
          Either::A(s) => s,
          Either::B(_) => &"<buffer>",
        },
      )
      .finish()
  }
}

impl From<BindingAssetSource> for rolldown_common::StrOrBytes {
  fn from(value: BindingAssetSource) -> Self {
    match value.inner {
      Either::A(s) => Self::Str(s),
      Either::B(buff) => Self::Bytes(buff.to_vec(), false),
    }
  }
}

impl From<rolldown_common::StrOrBytes> for BindingAssetSource {
  fn from(value: rolldown_common::StrOrBytes) -> Self {
    match value {
      rolldown_common::StrOrBytes::Str(s) => Self { inner: Either::A(s) },
      rolldown_common::StrOrBytes::ArcStr(s) => Self { inner: Either::A(s.to_string()) },
      rolldown_common::StrOrBytes::Bytes(buff, _) => Self { inner: Either::B(buff.into()) },
    }
  }
}
