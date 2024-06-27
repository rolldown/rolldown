use derivative::Derivative;
use napi::bindgen_prelude::Buffer;
use napi::Either;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Derivative)]
pub struct BindingAssetSource {
  #[napi(ts_type = "string | Uint8Array")]
  #[serde(skip_deserializing, default = "default_source")]
  pub inner: Either<String, Buffer>,
}

fn default_source() -> Either<String, Buffer> {
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

impl From<BindingAssetSource> for rolldown_common::AssetSource {
  fn from(value: BindingAssetSource) -> Self {
    match value.inner {
      Either::A(s) => Self::String(s),
      Either::B(buff) => Self::Buffer(buff.to_vec()),
    }
  }
}

impl From<rolldown_common::AssetSource> for BindingAssetSource {
  fn from(value: rolldown_common::AssetSource) -> Self {
    match value {
      rolldown_common::AssetSource::String(s) => Self { inner: Either::A(s) },
      rolldown_common::AssetSource::Buffer(buff) => Self { inner: Either::B(buff.into()) },
    }
  }
}
