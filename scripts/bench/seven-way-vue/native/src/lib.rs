//! Vize SFC compile, exposed as a tiny C ABI so we can be loaded via dlopen
//! by rolldown's native-lib plugin loader AND by the napi-side
//! `BenchVizeTransformer`. The ABI mirrors `rolldown_native_plugin_abi` byte
//! for byte (we deliberately don't depend on that crate to keep this cdylib
//! out of rolldown's link graph — see Cargo.toml).
//!
//! ABI version `1` matches `rolldown_native_plugin_abi::ABI_VERSION` at the
//! time of writing. If the rolldown ABI bumps, this file needs an update.
//!
//! Symbols exported:
//! - `rolldown_native_plugin_abi_version() -> u32`
//! - `rolldown_native_plugin_transform(source, id, *mut out) -> i32`
//! - `rolldown_native_plugin_drop_output(*mut out)`

use vize_atelier_sfc::{
    ScriptCompileOptions, SfcCompileOptions, SfcParseOptions, StyleCompileOptions,
    TemplateCompileOptions, compile_sfc, parse_sfc,
};

const ABI_VERSION: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NativeStr {
    pub ptr: *const u8,
    pub len: usize,
}

impl NativeStr {
    const EMPTY: Self = Self { ptr: std::ptr::null(), len: 0 };

    /// # Safety
    /// `self` must reference a valid UTF-8 buffer for `'a`.
    unsafe fn as_str<'a>(&self) -> &'a str {
        if self.len == 0 {
            return "";
        }
        unsafe {
            let bytes = std::slice::from_raw_parts(self.ptr, self.len);
            std::str::from_utf8_unchecked(bytes)
        }
    }
}

#[repr(C)]
pub struct TransformOutput {
    pub code: NativeStr,
    pub error: NativeStr,
    pub plugin_data: *mut core::ffi::c_void,
}

struct OwnedOutput {
    code: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn rolldown_native_plugin_abi_version() -> u32 {
    ABI_VERSION
}

/// # Safety
/// `out` must be a valid, writable `TransformOutput`. `source` and `id` must
/// reference valid UTF-8 buffers for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_native_plugin_transform(
    source: NativeStr,
    id: NativeStr,
    out: *mut TransformOutput,
) -> i32 {
    // SAFETY: caller upholds the lifetime contract.
    let id_str = unsafe { id.as_str() };
    let src = unsafe { source.as_str() };

    // Skip non-.vue modules — rolldown's runtime module flows through this
    // hook too and a stub would wipe out runtime helpers.
    if !id_str.ends_with(".vue") {
        // Echo the source back verbatim. The host will free it via drop_output.
        let owned = Box::new(OwnedOutput { code: src.to_string() });
        let code_native = NativeStr { ptr: owned.code.as_ptr(), len: owned.code.len() };
        let plugin_data = Box::into_raw(owned).cast::<core::ffi::c_void>();
        unsafe {
            (*out).code = code_native;
            (*out).error = NativeStr::EMPTY;
            (*out).plugin_data = plugin_data;
        }
        return 0;
    }

    let result = std::panic::catch_unwind(|| run_vize_compile(src, id_str));

    // On Vize panic or compile failure, emit a stub module instead of
    // propagating an error. The .vue source isn't valid TS for rolldown's
    // parser so "leave unchanged" wouldn't work. Matches the JS-side
    // utils variants.
    let code = match result {
        Ok(Ok(c)) => c,
        _ => "export default {};\n".to_string(),
    };

    let owned = Box::new(OwnedOutput { code });
    let code_native = NativeStr { ptr: owned.code.as_ptr(), len: owned.code.len() };
    let plugin_data = Box::into_raw(owned).cast::<core::ffi::c_void>();

    unsafe {
        (*out).code = code_native;
        (*out).error = NativeStr::EMPTY;
        (*out).plugin_data = plugin_data;
    }
    0
}

/// # Safety
/// `out` must point to a `TransformOutput` populated by a prior successful
/// call to `rolldown_native_plugin_transform` and not yet dropped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rolldown_native_plugin_drop_output(out: *mut TransformOutput) {
    if out.is_null() {
        return;
    }
    unsafe {
        let pd = (*out).plugin_data;
        if !pd.is_null() {
            drop(Box::from_raw(pd.cast::<OwnedOutput>()));
        }
        (*out).code = NativeStr::EMPTY;
        (*out).error = NativeStr::EMPTY;
        (*out).plugin_data = std::ptr::null_mut();
    }
}

fn run_vize_compile(source: &str, id: &str) -> Result<String, ()> {
    // Elk is uniformly `<script setup lang="ts">`. We pass `is_ts = true`
    // unconditionally — matches what plugin-vue does when the script block
    // declares lang="ts".
    let is_ts = true;
    let parse_opts = SfcParseOptions { filename: id.into(), ..Default::default() };
    let descriptor = parse_sfc(source, parse_opts).map_err(|_| ())?;

    let compile_opts = SfcCompileOptions {
        parse: SfcParseOptions { filename: id.into(), ..Default::default() },
        script: ScriptCompileOptions { id: Some(id.into()), is_ts, ..Default::default() },
        template: TemplateCompileOptions { id: Some(id.into()), is_ts, ..Default::default() },
        style: StyleCompileOptions { id: id.into(), ..Default::default() },
        ..Default::default()
    };

    // Vize's `SfcCompileResult.code` is a `compact_str::CompactString` (aliased
    // to `String` inside `vize_carton`). Convert to std::string::String at the
    // FFI boundary so the rest of the cdylib can hand it back as plain UTF-8
    // bytes without leaking the CompactString type.
    compile_sfc(&descriptor, compile_opts).map(|r| r.code.to_string()).map_err(|_| ())
}
