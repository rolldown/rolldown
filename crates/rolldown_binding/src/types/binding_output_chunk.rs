use std::sync::Arc;

use napi::{
  Either, Env,
  bindgen_prelude::{Null, ObjectFinalize},
};
use napi_derive::napi;
use rolldown_sourcemap::SourceMap;

use super::{binding_rendered_chunk::BindingModules, binding_sourcemap::BindingSourcemap};

#[napi(custom_finalize)]
pub struct BindingOutputChunk {
  inner: Arc<rolldown_common::OutputChunk>,
  /// What V8 was told this object keeps alive.
  external_bytes: i64,
}

#[napi]
impl BindingOutputChunk {
  pub fn new(inner: Arc<rolldown_common::OutputChunk>) -> Self {
    Self { inner, external_bytes: 0 }
  }

  /// V8 cannot see this memory, so without this it feels no pressure to collect this object.
  #[napi(enumerable = false)]
  pub fn track_external_memory(&mut self, env: &Env) -> napi::Result<()> {
    if self.external_bytes == 0 {
      let bytes =
        i64::try_from(self.inner.code.len() + self.inner.map.as_ref().map_or(0, sourcemap_bytes))
          .unwrap_or(i64::MAX);
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

  #[napi]
  pub fn get_is_entry(&self) -> bool {
    self.inner.is_entry
  }

  #[napi]
  pub fn get_is_dynamic_entry(&self) -> bool {
    self.inner.is_dynamic_entry
  }

  #[napi]
  pub fn get_facade_module_id(&self) -> Option<&str> {
    self.inner.facade_module_id.as_deref()
  }

  #[napi]
  pub fn get_module_ids(&self) -> Vec<&str> {
    self.inner.module_ids.iter().map(AsRef::as_ref).collect()
  }

  #[napi]
  pub fn get_exports(&self) -> Vec<&str> {
    self.inner.exports.iter().map(AsRef::as_ref).collect()
  }

  // RenderedChunk
  #[napi]
  pub fn get_file_name(&self) -> &str {
    &self.inner.filename
  }

  #[napi]
  pub fn get_modules(&self) -> BindingModules {
    (&self.inner.modules).into()
  }

  #[napi]
  pub fn get_imports(&self) -> Vec<&str> {
    self.inner.imports.iter().map(AsRef::as_ref).collect()
  }

  #[napi]
  pub fn get_dynamic_imports(&self) -> Vec<&str> {
    self.inner.dynamic_imports.iter().map(AsRef::as_ref).collect()
  }

  // OutputChunk
  #[napi]
  pub fn get_immutable(&self) -> bool {
    self.inner.immutable
  }

  #[napi]
  pub fn get_code(&self) -> &str {
    &self.inner.code
  }

  /// Identity of the shared chunk data behind this object. Two `BindingOutputChunk` objects with the same
  /// identity share the same data, so a JavaScript string made from `getCode()` of one is valid for the other.
  /// The identity is the address of that data. It stays unique while a `BindingOutputChunk` keeps the data alive.
  #[napi]
  pub fn get_code_identity(&self) -> i64 {
    let address = Arc::as_ptr(&self.inner).addr();
    i64::try_from(address).unwrap_or(i64::MAX)
  }

  #[napi]
  // TODO: claude code - Cannot change to Option<&str>: performs JSON serialization via to_json_string()
  pub fn get_map(&self) -> Option<String> {
    self.inner.map.as_ref().map(SourceMap::to_json_string)
  }

  #[napi]
  pub fn get_sourcemap_file_name(&self) -> Option<&str> {
    self.inner.sourcemap_filename.as_deref()
  }

  #[napi]
  pub fn get_preliminary_file_name(&self) -> &str {
    &self.inner.preliminary_filename
  }

  #[napi]
  pub fn get_name(&self) -> &str {
    &self.inner.name
  }
}

/// What a `generateBundle` or `writeBundle` hook changed in an output chunk. Rust fills omitted
/// fields from the existing chunk and clears `immutable` on a rename without an explicit value.
/// See `collectChunkChange` in
/// `packages/rolldown/src/utils/transform-to-rollup-output.ts`.
#[napi_derive::napi(object, object_to_js = false)]
pub struct JsOutputChunk {
  pub filename: String,
  /// Always sent. Besides its value, it tells a chunk apart from a `JsOutputAsset`, which has no
  /// `isEntry`, when napi picks a side of `Either<JsOutputChunk, JsOutputAsset>`.
  pub is_entry: bool,
  pub immutable: Option<bool>,
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
  let immutable =
    js_chunk.immutable.unwrap_or_else(|| chunk.immutable && chunk.filename == filename);

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
    chunk.immutable = immutable;
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
      immutable,
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

impl ObjectFinalize for BindingOutputChunk {
  fn finalize(mut self, env: Env) -> napi::Result<()> {
    self.untrack_external_memory(&env)?;
    crate::memory_reclaim::notify();
    Ok(())
  }
}

fn sourcemap_bytes(map: &SourceMap) -> usize {
  let contents: usize = map.get_source_contents().flatten().map(str::len).sum();
  contents + map.get_tokens().len() * size_of::<oxc_sourcemap::Token>()
}
