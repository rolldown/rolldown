use std::sync::Arc;

use rolldown_common::RollupRenderedChunk;

use crate::types::binding_rendered_module::BindingRenderedModule;

use super::external_memory_status::ExternalMemoryStatus;

#[napi_derive::napi]
#[derive(Debug)]
pub struct BindingRenderedChunk {
  inner: Option<Arc<RollupRenderedChunk>>,
}

#[napi_derive::napi]
impl BindingRenderedChunk {
  pub fn new(inner: Arc<RollupRenderedChunk>) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<RollupRenderedChunk>> {
    self.inner.as_ref().ok_or_else(|| {
      napi::Error::from_reason(
        "Memory has been freed: this rendered chunk's native data was eagerly released after its hook invocation settled. Copy the fields you need during the hook.",
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
  pub fn get_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.name)
  }

  #[napi(getter)]
  pub fn get_is_entry(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.is_entry)
  }

  #[napi(getter)]
  pub fn get_is_dynamic_entry(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.is_dynamic_entry)
  }

  #[napi(getter)]
  pub fn get_facade_module_id(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.facade_module_id.as_deref())
  }

  #[napi(getter)]
  pub fn get_module_ids(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.module_ids.iter().map(AsRef::as_ref).collect())
  }

  #[napi(getter)]
  pub fn get_exports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.exports.iter().map(AsRef::as_ref).collect())
  }

  #[napi(getter)]
  pub fn get_file_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.filename)
  }

  #[napi(getter)]
  pub fn get_modules(&self) -> napi::Result<BindingModules> {
    Ok((&self.try_get_inner()?.modules).into())
  }

  #[napi(getter)]
  pub fn get_imports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.imports.iter().map(AsRef::as_ref).collect())
  }

  #[napi(getter)]
  pub fn get_dynamic_imports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.dynamic_imports.iter().map(AsRef::as_ref).collect())
  }
}

#[napi_derive::napi(object, object_from_js = false)]
#[derive(Default, Debug, Clone)]
pub struct BindingModules {
  pub values: Vec<BindingRenderedModule>,
  pub keys: Vec<String>,
}

impl From<&rolldown_common::Modules> for BindingModules {
  fn from(modules: &rolldown_common::Modules) -> Self {
    let values = modules.values.iter().map(|x| BindingRenderedModule::new(Arc::clone(x))).collect();
    let keys = modules.keys.iter().map(ToString::to_string).collect();
    Self { values, keys }
  }
}
