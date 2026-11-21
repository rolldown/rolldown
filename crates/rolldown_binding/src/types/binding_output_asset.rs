use std::sync::Arc;

use napi::bindgen_prelude::ObjectFinalize;
use napi::{
  Env,
  bindgen_prelude::{BufferSlice, JsObjectValue, Object},
};
use napi_derive::napi;

use crate::{
  options::plugin::types::binding_asset_source::BindingAssetSource,
  types::external_memory_status::ExternalMemoryStatus,
};

#[napi(custom_finalize)]
pub struct BindingOutputAsset {
  inner: Option<Arc<rolldown_common::OutputAsset>>,
  /// What V8 was told this object keeps alive.
  external_bytes: i64,
}

#[napi]
impl BindingOutputAsset {
  pub fn new(inner: Arc<rolldown_common::OutputAsset>) -> Self {
    Self { inner: Some(inner), external_bytes: 0 }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<rolldown_common::OutputAsset>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed by `freeExternalMemory()`. Cannot access properties. To prevent this, use `freeExternalMemory(handle, true)` with `keepDataAlive`.",
      )
    })
  }

  /// V8 cannot see this memory, so without this it feels no pressure to collect this object.
  /// See internal-docs/memory-reclaim/implementation.md
  #[napi(enumerable = false)]
  pub fn track_external_memory(&mut self, env: &Env) -> napi::Result<()> {
    if self.external_bytes == 0
      && let Some(inner) = &self.inner
    {
      let bytes = i64::try_from(inner.source.as_bytes().len()).unwrap_or(i64::MAX);
      env.adjust_external_memory(bytes)?;
      self.external_bytes = bytes;
    }
    Ok(())
  }

  fn untrack_external_memory(&mut self, env: &Env) -> napi::Result<()> {
    if self.external_bytes != 0 {
      env.adjust_external_memory(-self.external_bytes)?;
      self.external_bytes = 0;
    }
    Ok(())
  }

  #[napi(enumerable = false)]
  pub fn drop_inner(&mut self, env: &Env) -> napi::Result<ExternalMemoryStatus> {
    self.untrack_external_memory(env)?;
    crate::memory_reclaim::notify();
    Ok(self.release_inner())
  }

  fn release_inner(&mut self) -> ExternalMemoryStatus {
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

impl ObjectFinalize for BindingOutputAsset {
  fn finalize(mut self, env: Env) -> napi::Result<()> {
    self.untrack_external_memory(&env)?;
    crate::memory_reclaim::notify();
    Ok(())
  }
}
