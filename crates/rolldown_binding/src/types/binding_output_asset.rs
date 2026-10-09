use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use napi::bindgen_prelude::BufferSlice;
use napi::{
  Env,
  bindgen_prelude::{JsObjectValue, Object},
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
        #[cfg(not(target_family = "wasm"))]
        {
          source.set_named_property("inner", BufferSlice::copy_from(env, value)?)?;
        }
        // Wasm needs a JS-owned copy; see `js_owned_uint8_array`.
        #[cfg(target_family = "wasm")]
        {
          source.set_named_property("inner", js_owned_uint8_array(env, value.as_slice())?)?;
        }
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

/// Create a `Uint8Array` holding a JS-owned copy of `data`, with no native memory handed to
/// a GC finalizer (a threadless WASI host may never run GC finalizers, so it would leak).
///
/// On emnapi, `napi_create_external_arraybuffer` with a NULL `finalize_cb` copies the bytes
/// into a fresh JS `ArrayBuffer` synchronously, so `data` only needs to live for this call.
/// Not `napi_create_arraybuffer` + copy: emnapi hands back a Wasm-side mirror, so the copied
/// bytes never reach JavaScript.
///
/// Returns `slice()`, not the external view: emnapi remembers the external buffer's creation
/// address, so passing that object back into the binding would read freed memory once
/// `dropInner` released the asset. The `slice()` copy has no such entry.
#[cfg(target_family = "wasm")]
fn js_owned_uint8_array<'env>(
  env: &'env Env,
  data: &[u8],
) -> napi::Result<napi::bindgen_prelude::Unknown<'env>> {
  use napi::bindgen_prelude::{FromNapiValue, JsObjectValue, Object, Unknown};

  let len = data.len();
  let mut array_buffer = std::ptr::null_mut();
  if len == 0 {
    // An empty external arraybuffer is created detached on emnapi and typed
    // arrays cannot view detached buffers; make a plain empty one instead.
    napi::check_status!(
      unsafe {
        napi::sys::napi_create_arraybuffer(env.raw(), 0, std::ptr::null_mut(), &mut array_buffer)
      },
      "Failed to create the empty JS-owned ArrayBuffer for an asset source"
    )?;
    let mut typed_array = std::ptr::null_mut();
    napi::check_status!(
      unsafe {
        napi::sys::napi_create_typedarray(
          env.raw(),
          napi::sys::TypedarrayType::uint8_array,
          0,
          array_buffer,
          0,
          &mut typed_array,
        )
      },
      "Failed to create the empty Uint8Array for an asset source"
    )?;
    return unsafe { Unknown::from_napi_value(env.raw(), typed_array) };
  }

  napi::check_status!(
    unsafe {
      napi::sys::napi_create_external_arraybuffer(
        env.raw(),
        data.as_ptr().cast_mut().cast(),
        len,
        None,
        std::ptr::null_mut(),
        &mut array_buffer,
      )
    },
    "Failed to create the JS-owned ArrayBuffer copy of an asset source"
  )?;
  let mut typed_array = std::ptr::null_mut();
  napi::check_status!(
    unsafe {
      napi::sys::napi_create_typedarray(
        env.raw(),
        napi::sys::TypedarrayType::uint8_array,
        len,
        array_buffer,
        0,
        &mut typed_array,
      )
    },
    "Failed to create the Uint8Array view over the JS-owned asset source"
  )?;
  // Detach the result from the recorded native address (see above).
  let view: Object = unsafe { Object::from_napi_value(env.raw(), typed_array)? };
  let slice_fn: napi::bindgen_prelude::Function<(), Unknown> = view.get_named_property("slice")?;
  slice_fn.apply(view, ())
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
