use std::sync::Arc;

use napi::{
  Env,
  bindgen_prelude::{BufferSlice, JsObjectValue, Object},
};
use napi_derive::napi;

use crate::{
  options::plugin::types::binding_asset_source::BindingAssetSource,
  types::external_memory_status::{ExternalMemoryStatus, release_arc},
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
    release_arc(&mut self.inner)
  }

  #[napi]
  pub fn get_file_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.filename)
  }

  #[napi]
  pub fn get_original_file_name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.original_file_names.first().map(AsRef::as_ref))
  }

  #[napi]
  pub fn get_original_file_names(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.original_file_names.iter().map(AsRef::as_ref).collect())
  }

  #[napi(ts_return_type = "BindingAssetSource")]
  pub fn get_source<'env>(&self, env: &'env Env) -> napi::Result<Object<'env>> {
    let mut source = Object::new(env)?;
    match &self.try_get_inner()?.source {
      rolldown_common::StrOrBytes::Str(value) => {
        source.set_named_property("inner", env.create_string(value)?)?;
      }
      rolldown_common::StrOrBytes::Bytes(value) => {
        source.set_named_property("inner", BufferSlice::copy_from(env, value)?)?;
      }
    }
    Ok(source)
  }

  #[napi]
  pub fn get_name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.names.first().map(AsRef::as_ref))
  }

  #[napi]
  pub fn get_names(&self) -> napi::Result<Vec<&str>> {
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
