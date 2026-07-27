use std::sync::Arc;

#[cfg(not(target_family = "wasm"))]
use napi::bindgen_prelude::Uint8ArraySlice;
use napi::{
  Env,
  bindgen_prelude::{JsObjectValue, Object},
};
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
          source.set_named_property("inner", Uint8ArraySlice::from_data(env, value.as_slice())?)?;
        }
        // On Wasm, `Uint8ArraySlice::from_data` copies the bytes into a Rust
        // `Vec` and hands it to `napi_create_external_arraybuffer`, whose
        // buffer emnapi copies to JavaScript AGAIN while the `Vec` itself is
        // `mem::forget`-ed and owned by a GC finalizer. Engines that do not
        // reliably run finalizers for Wasm-held externals (workerd never
        // does) then leak one copy of the asset bytes per build. Hand emnapi
        // the asset's own bytes instead: it copies them into a JS-owned
        // `ArrayBuffer` synchronously and, with no finalizer to register,
        // nothing on the native side outlives this call.
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

/// Create a `Uint8Array` over a JS-owned copy of `data` without giving any
/// native memory to a GC finalizer.
///
/// `napi_create_external_arraybuffer` on emnapi copies the referenced Wasm
/// bytes into a fresh JS `ArrayBuffer` synchronously (JavaScript cannot alias
/// Wasm linear memory), and a NULL `finalize_cb` skips finalizer registration
/// entirely, so nothing native outlives this call. `data` only needs to stay
/// valid for the duration of the call — here it borrows the asset's own bytes.
///
/// Deliberately NOT `napi_create_arraybuffer` + copy (napi-rs's
/// `no_external_buffers_allowed` fallback): emnapi hands back a pointer to a
/// runtime-allocated Wasm-side MIRROR of the JS buffer, so the copied bytes
/// never reach JavaScript and the mirror itself is freed by a
/// FinalizationRegistry — the exact never-runs-on-workerd mechanism this
/// function exists to avoid.
///
/// Known edge: emnapi remembers (in a WeakMap) that this `ArrayBuffer` once
/// lived at `data`'s address. Passing the SAME `Uint8Array` object back into
/// the binding after the asset's native payload was freed reads through that
/// stale address. The lazy native path has the identical edge once GC frees
/// its forgotten `Vec`; consumers re-emitting a freed build's bytes should
/// pass a fresh copy.
#[cfg(target_family = "wasm")]
fn js_owned_uint8_array<'env>(
  env: &'env Env,
  data: &[u8],
) -> napi::Result<napi::bindgen_prelude::Unknown<'env>> {
  use napi::bindgen_prelude::FromNapiValue;

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
  } else {
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
  }
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
  unsafe { napi::bindgen_prelude::Unknown::from_napi_value(env.raw(), typed_array) }
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
