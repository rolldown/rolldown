use std::sync::Arc;

use napi_derive::napi;

use crate::{
  options::plugin::types::binding_asset_source::BindingAssetSource,
  types::external_memory_status::ExternalMemoryStatus,
};

#[napi]
pub struct BindingOutputAsset {
  inner: Option<Arc<rolldown_common::OutputAsset>>,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: Arc<rolldown_common::OutputAsset>) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<rolldown_common::OutputAsset>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed by `freeExternalMemory()`. Cannot access properties. To prevent this, use `freeExternalMemory(handle, true)` with `keepDataAlive`.",
      )
    })
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self) -> ExternalMemoryStatus {
    match self.inner.take() {
      None => ExternalMemoryStatus {
        freed: false,
        reason: Some("Memory has already been freed".to_string()),
      },
      Some(arc) => {
        let strong_count = Arc::strong_count(&arc);
        if strong_count > 1 {
          // Drop our reference, but others exist
          // Arc drops here automatically
          ExternalMemoryStatus {
            freed: false,
            reason: Some(format!(
              "Data has been dropped, but there are {} other strong reference(s) referring to this data on the native side, so the memory may not be released.",
              strong_count - 1
            )),
          }
        } else {
          // Last reference - memory will be freed
          // Arc drops here automatically
          ExternalMemoryStatus { freed: true, reason: None }
        }
      }
    }
  }

  #[napi(getter)]
  pub fn file_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.filename)
  }

  #[napi(getter)]
  pub fn original_file_name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.original_file_names.first().map(AsRef::as_ref))
  }

  #[napi(getter)]
  pub fn original_file_names(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.original_file_names.iter().map(AsRef::as_ref).collect())
  }

  #[napi(getter)]
  pub fn source(&self) -> napi::Result<BindingAssetSource> {
    Ok(self.try_get_inner()?.source.clone().into())
  }

  #[napi(getter)]
  pub fn name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.names.first().map(AsRef::as_ref))
  }

  #[napi(getter)]
  pub fn names(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.names.iter().map(AsRef::as_ref).collect())
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct JsOutputAsset {
  pub names: Vec<String>,
  pub original_file_names: Vec<String>,
  pub filename: String,
  pub source: BindingAssetSource,
}

impl From<JsOutputAsset> for rolldown_common::OutputAsset {
  fn from(asset: JsOutputAsset) -> Self {
    Self {
      names: asset.names,
      original_file_names: asset.original_file_names,
      filename: asset.filename.into(),
      source: asset.source.into(),
    }
  }
}
