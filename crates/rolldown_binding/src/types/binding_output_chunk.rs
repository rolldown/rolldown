use std::sync::Arc;

use napi::{Either, bindgen_prelude::Null};
use napi_derive::napi;
use rolldown_sourcemap::SourceMap;

use super::{
  binding_rendered_chunk::BindingModules,
  binding_sourcemap::BindingSourcemap,
  external_memory_status::{ExternalMemoryStatus, release_arc},
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
    release_arc(&mut self.inner)
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

  /// Identity of the shared chunk data behind this object. Two `BindingOutputChunk` objects with the same
  /// identity share the same data, so a JavaScript string made from `getCode()` of one is valid for the other.
  /// The identity is the address of that data. It stays unique while a `BindingOutputChunk` keeps the data alive.
  #[napi]
  pub fn get_code_identity(&self) -> napi::Result<i64> {
    let address = Arc::as_ptr(self.try_get_inner()?).addr();
    Ok(i64::try_from(address).unwrap_or(i64::MAX))
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

/// What a `generateBundle` or `writeBundle` hook changed in an output chunk. A field that is left
/// out is a property the plugin did not change, so the chunk keeps its value, which is never copied
/// through JavaScript. See `collectChunkChange` in
/// `packages/rolldown/src/utils/transform-to-rollup-output.ts`.
#[napi_derive::napi(object, object_to_js = false)]
pub struct JsOutputChunk {
  pub filename: String,
  /// Always sent. Besides its value, it tells a chunk apart from a `JsOutputAsset`, which has no
  /// `isEntry`, when napi picks a side of `Either<JsOutputChunk, JsOutputAsset>`.
  pub is_entry: bool,
  pub code: Option<String>,
  /// `null` removes the map.
  pub map: Option<Either<BindingSourcemap, Null>>,
  pub imports: Option<Vec<String>>,
  pub dynamic_imports: Option<Vec<String>>,
}

pub fn update_output_chunk(
  chunk: &mut Arc<rolldown_common::OutputChunk>,
  js_chunk: JsOutputChunk,
) -> anyhow::Result<()> {
  let map = match js_chunk.map {
    Some(Either::A(map)) => Some(Some(map.try_into()?)),
    Some(Either::B(_)) => Some(None),
    None => None,
  };
  let imports = js_chunk.imports.map(|imports| imports.into_iter().map(Into::into).collect());
  let dynamic_imports =
    js_chunk.dynamic_imports.map(|imports| imports.into_iter().map(Into::into).collect());
  let filename = js_chunk.filename.into();

  if let Some(chunk) = Arc::get_mut(chunk) {
    if let Some(code) = js_chunk.code {
      chunk.code = code;
    }
    if let Some(map) = map {
      chunk.map = map;
    }
    if let Some(imports) = imports {
      chunk.imports = imports;
    }
    if let Some(dynamic_imports) = dynamic_imports {
      chunk.dynamic_imports = dynamic_imports;
    }
    chunk.is_entry = js_chunk.is_entry; // used by nuxt
    chunk.filename = filename;
  } else {
    let old_chunk = Arc::as_ref(chunk);
    *chunk = Arc::new(rolldown_common::OutputChunk {
      name: old_chunk.name.clone(),
      is_entry: js_chunk.is_entry, // used by nuxt
      is_dynamic_entry: old_chunk.is_dynamic_entry,
      facade_module_id: old_chunk.facade_module_id.clone(),
      module_ids: old_chunk.module_ids.clone(),
      exports: old_chunk.exports.clone(),
      filename,
      modules: old_chunk.modules.clone(),
      imports: imports.unwrap_or_else(|| old_chunk.imports.clone()),
      dynamic_imports: dynamic_imports.unwrap_or_else(|| old_chunk.dynamic_imports.clone()),
      code: js_chunk.code.unwrap_or_else(|| old_chunk.code.clone()),
      map: map.unwrap_or_else(|| old_chunk.map.clone()),
      sourcemap_filename: old_chunk.sourcemap_filename.clone(),
      preliminary_filename: old_chunk.preliminary_filename.clone(),
    });
  }
  Ok(())
}
