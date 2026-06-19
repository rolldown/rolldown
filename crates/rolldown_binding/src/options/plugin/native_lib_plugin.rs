use std::borrow::Cow;
use std::sync::Arc;

use anyhow::{Context as _, anyhow};
use libloading::{Library, Symbol};
use rolldown_native_plugin_abi::{
  ABI_VERSION, FnAbiVersion, FnDropOutput, FnTransform, NativeStr, SYM_ABI_VERSION,
  SYM_DROP_OUTPUT, SYM_TRANSFORM, TransformOutput,
};
use rolldown_plugin::{
  HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin, __inner::SharedPluginable,
};

pub struct NativeLibPlugin {
  name: String,
  // Keep the Library alive for the lifetime of this plugin. The fn pointers
  // below are only valid while `_lib` is loaded.
  _lib: Arc<Library>,
  transform: FnTransform,
  drop_output: FnDropOutput,
}

impl std::fmt::Debug for NativeLibPlugin {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("NativeLibPlugin")
      .field("name", &self.name)
      .field("_lib", &"<libloading::Library>")
      .field("transform", &(self.transform as *const ()))
      .field("drop_output", &(self.drop_output as *const ()))
      .finish()
  }
}

impl NativeLibPlugin {
  pub fn load(name: String, path: &str) -> napi::Result<Self> {
    // SAFETY: we trust the user-supplied path. dlopen executes the library's
    // initializers, which is inherently unsafe.
    let lib = unsafe { Library::new(path) }
      .with_context(|| format!("failed to dlopen native plugin: {path}"))
      .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

    let (transform, drop_output) = unsafe {
      let abi_version: Symbol<FnAbiVersion> = lib
        .get(SYM_ABI_VERSION.as_bytes())
        .with_context(|| format!("missing symbol {SYM_ABI_VERSION} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let v = abi_version();
      if v != ABI_VERSION {
        return Err(napi::Error::from_reason(format!(
          "native plugin {path} reports ABI version {v}, host expects {ABI_VERSION}"
        )));
      }

      let transform: Symbol<FnTransform> = lib
        .get(SYM_TRANSFORM.as_bytes())
        .with_context(|| format!("missing symbol {SYM_TRANSFORM} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;
      let drop_output: Symbol<FnDropOutput> = lib
        .get(SYM_DROP_OUTPUT.as_bytes())
        .with_context(|| format!("missing symbol {SYM_DROP_OUTPUT} in {path}"))
        .map_err(|e| napi::Error::from_reason(format!("{e:#}")))?;

      (*transform, *drop_output)
    };

    Ok(Self { name, _lib: Arc::new(lib), transform, drop_output })
  }

  pub fn into_shared(self) -> SharedPluginable {
    Arc::new(self)
  }
}

impl Plugin for NativeLibPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Owned(self.name.clone())
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let source = NativeStr { ptr: args.code.as_ptr(), len: args.code.len() };
    let id_bytes = args.id.as_bytes();
    let id = NativeStr { ptr: id_bytes.as_ptr(), len: id_bytes.len() };

    let mut out = TransformOutput::ZEROED;
    // SAFETY: the plugin's `transform` is thread-safe per ABI contract; `out`
    // is a valid pointer to writable storage we own; `source`/`id` live until
    // this call returns.
    let rc = unsafe { (self.transform)(source, id, &raw mut out) };

    if rc != 0 {
      let msg = if out.error.len > 0 {
        // SAFETY: ABI contract says `error` (when non-empty) is valid UTF-8 until drop_output runs.
        unsafe { out.error.as_str() }.to_owned()
      } else {
        format!("native plugin returned error code {rc}")
      };
      unsafe { (self.drop_output)(&raw mut out) };
      return Err(anyhow!(msg));
    }

    // SAFETY: ABI contract says `code` is valid UTF-8 until drop_output runs.
    let owned: String = unsafe { out.code.as_str() }.to_owned();
    unsafe { (self.drop_output)(&raw mut out) };

    Ok(Some(HookTransformOutput {
      code: Some(owned),
      map: HookTransformOutputMap::Omitted,
      side_effects: None,
      module_type: None,
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }
}
