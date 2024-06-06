use derivative::Derivative;
use napi::bindgen_prelude::Buffer;
use serde::Deserialize;

#[napi_derive::napi(object)]
#[derive(Deserialize, Default, Derivative)]
pub struct BindingAssetSource {
  pub r#type: String,
  #[napi(ts_type = "Uint8Array")]
  #[serde(skip_deserializing)]
  pub source: Buffer,
}

impl std::fmt::Debug for BindingAssetSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BindingAssetSource")
      .field("r#type", &self.r#type)
      .field("source", &"<Buffer>")
      .finish()
  }
}

impl From<BindingAssetSource> for rolldown_common::AssetSource {
  fn from(value: BindingAssetSource) -> Self {
    match value.r#type.as_str() {
      "string" => Self::String(String::from_utf8_lossy(&value.source).to_string()),
      "buffer" => Self::Buffer(value.source.to_vec()),
      _ => unreachable!("unknown asset source type: {}", value.r#type),
    }
  }
}

impl From<rolldown_common::AssetSource> for BindingAssetSource {
  fn from(value: rolldown_common::AssetSource) -> Self {
    match value {
      rolldown_common::AssetSource::String(s) => {
        Self { r#type: "string".to_string(), source: s.into() }
      }
      rolldown_common::AssetSource::Buffer(buff) => {
        Self { r#type: "buffer".to_string(), source: buff.into() }
      }
    }
  }
}
