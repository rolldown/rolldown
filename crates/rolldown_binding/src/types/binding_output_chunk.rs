use std::{collections::HashMap, sync::Arc};

use napi_derive::napi;
use rolldown_sourcemap::SourceMap;
use rustc_hash::FxBuildHasher;

use super::{
  binding_rendered_chunk::BindingModules, binding_rendered_module::BindingRenderedModule,
  binding_sourcemap::BindingSourcemap, external_memory_status::ExternalMemoryStatus,
};

#[napi]
pub struct BindingOutputChunk {
  inner: Option<Arc<rolldown_common::OutputChunk>>,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: Arc<rolldown_common::OutputChunk>) -> Self {
    Self { inner: Some(inner) }
  }

  fn try_get_inner(&self) -> napi::Result<&Arc<rolldown_common::OutputChunk>> {
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

  #[napi]
  pub fn get_is_entry(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.is_entry)
  }

  #[napi]
  pub fn get_is_dynamic_entry(&self) -> napi::Result<bool> {
    Ok(self.try_get_inner()?.is_dynamic_entry)
  }

  #[napi]
  pub fn get_facade_module_id(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.facade_module_id.as_deref())
  }

  #[napi]
  pub fn get_module_ids(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.module_ids.iter().map(AsRef::as_ref).collect())
  }

  #[napi]
  pub fn get_exports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.exports.iter().map(AsRef::as_ref).collect())
  }

  // RenderedChunk
  #[napi]
  pub fn get_file_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.filename)
  }

  #[napi]
  pub fn get_modules(&self) -> napi::Result<BindingModules> {
    Ok((&self.try_get_inner()?.modules).into())
  }

  #[napi]
  pub fn get_imports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.imports.iter().map(AsRef::as_ref).collect())
  }

  #[napi]
  pub fn get_dynamic_imports(&self) -> napi::Result<Vec<&str>> {
    Ok(self.try_get_inner()?.dynamic_imports.iter().map(AsRef::as_ref).collect())
  }

  // OutputChunk
  #[napi]
  pub fn get_code(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.code)
  }

  #[napi]
  // TODO: claude code - Cannot change to Option<&str>: performs JSON serialization via to_json_string()
  pub fn get_map(&self) -> napi::Result<Option<String>> {
    Ok(self.try_get_inner()?.map.as_ref().map(SourceMap::to_json_string))
  }

  #[napi]
  pub fn get_sourcemap_file_name(&self) -> napi::Result<Option<&str>> {
    Ok(self.try_get_inner()?.sourcemap_filename.as_deref())
  }

  #[napi]
  pub fn get_preliminary_file_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.preliminary_filename)
  }

  #[napi]
  pub fn get_name(&self) -> napi::Result<&str> {
    Ok(&self.try_get_inner()?.name)
  }
}

#[napi_derive::napi(object, object_to_js = false)]
pub struct JsOutputChunk {
  // PreRenderedChunk
  pub name: String,
  pub is_entry: bool,
  pub is_dynamic_entry: bool,
  pub facade_module_id: Option<String>,
  pub module_ids: Vec<String>,
  pub exports: Vec<String>,
  // RenderedChunk
  pub filename: String,
  pub modules: HashMap<String, BindingRenderedModule, FxBuildHasher>,
  pub imports: Vec<String>,
  pub dynamic_imports: Vec<String>,
  // OutputChunk
  pub code: String,
  pub map: Option<BindingSourcemap>,
  pub sourcemap_filename: Option<String>,
  pub preliminary_filename: String,
}

pub fn update_output_chunk(
  chunk: &mut Arc<rolldown_common::OutputChunk>,
  js_chunk: JsOutputChunk,
) -> anyhow::Result<()> {
  let old_chunk = (**chunk).clone();
  *chunk = Arc::new(rolldown_common::OutputChunk {
    code: js_chunk.code,
    map: js_chunk.map.map(TryInto::try_into).transpose()?,
    imports: js_chunk.imports.into_iter().map(Into::into).collect(),
    dynamic_imports: js_chunk.dynamic_imports.into_iter().map(Into::into).collect(),
    is_entry: js_chunk.is_entry, // used by nuxt
    filename: js_chunk.filename.into(),
    ..old_chunk
  });
  Ok(())
}
