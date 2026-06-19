//! Stable C ABI for rolldown native plugins loaded via dlopen.
//!
//! A plugin is a shared library (`.dylib`/`.so`/`.dll`) that exports a small,
//! fixed set of `extern "C"` symbols. Rolldown loads the library at startup
//! and dispatches transform calls directly from its worker threads via raw
//! function pointers — no JS, no napi, no ThreadsafeFunction.
//!
//! # ABI guarantees
//!
//! The types in this crate are `#[repr(C)]` so their field layout is stable
//! across Rust versions and compilers (under the same target ABI). Pointer
//! and `usize` widths follow the target's platform. There is no Rust-specific
//! data on the wire.
//!
//! # Symbol contract
//!
//! Every plugin must export these symbols with C linkage:
//!
//! - `rolldown_native_plugin_abi_version() -> u32` returns the plugin's ABI
//!   version. Rolldown refuses to load plugins whose version differs from
//!   `ABI_VERSION`.
//! - `rolldown_native_plugin_transform(source, id, *mut out) -> i32` runs the
//!   transform. Returns 0 on success; non-zero on error.
//! - `rolldown_native_plugin_drop_output(*mut out)` releases the buffers in
//!   `out`. The host calls this exactly once per successful `transform`.
//!
//! # Lifetime rules
//!
//! - `source`/`id` are borrowed for the duration of the `transform` call only.
//!   The plugin must NOT retain pointers into them after returning.
//! - `out.code` (and `out.error`, if present) must remain valid until the host
//!   calls `rolldown_native_plugin_drop_output`. The host calls drop exactly
//!   once per successful `transform`.
//! - Buffers in `out` are owned and freed by the plugin (so allocator
//!   mismatches between host and plugin are impossible).

/// Bump on every breaking change to the symbol set or the types below.
pub const ABI_VERSION: u32 = 1;

/// Borrowed view of a UTF-8 byte slice. Equivalent layout to
/// `{ const u8 *ptr; size_t len; }`. The pointer may be null iff `len == 0`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NativeStr {
  pub ptr: *const u8,
  pub len: usize,
}

impl NativeStr {
  pub const EMPTY: Self = Self { ptr: std::ptr::null(), len: 0 };

  /// # Safety
  /// `self` must reference a valid UTF-8 buffer for `'a`.
  pub unsafe fn as_str<'a>(&self) -> &'a str {
    if self.len == 0 {
      return "";
    }
    unsafe {
      let bytes = std::slice::from_raw_parts(self.ptr, self.len);
      std::str::from_utf8_unchecked(bytes)
    }
  }
}

// SAFETY: the pointers in NativeStr only carry references for the duration
// of a single call; the buffer's owner upholds Send/Sync.
unsafe impl Send for NativeStr {}
unsafe impl Sync for NativeStr {}

/// Out-parameter for `transform`. On success the plugin populates `code` with
/// a pointer to the transformed source. On error, `code` is left as
/// `NativeStr::EMPTY` and `error` points at a UTF-8 error message (optional —
/// may also be empty).
///
/// `plugin_data` is an opaque pointer the plugin may use to track per-output
/// state (e.g. a pointer to a `Box<String>` so `drop_output` can reclaim it).
/// Host code must not interpret `plugin_data`.
#[repr(C)]
pub struct TransformOutput {
  pub code: NativeStr,
  pub error: NativeStr,
  pub plugin_data: *mut core::ffi::c_void,
}

impl TransformOutput {
  pub const ZEROED: Self = Self {
    code: NativeStr::EMPTY,
    error: NativeStr::EMPTY,
    plugin_data: std::ptr::null_mut(),
  };
}

unsafe impl Send for TransformOutput {}
unsafe impl Sync for TransformOutput {}

/// Function-pointer types for the three symbols a plugin exports. Useful for
/// hosts that resolve symbols via `dlsym`/`libloading`.
pub type FnAbiVersion = unsafe extern "C" fn() -> u32;
pub type FnTransform =
  unsafe extern "C" fn(source: NativeStr, id: NativeStr, out: *mut TransformOutput) -> i32;
pub type FnDropOutput = unsafe extern "C" fn(out: *mut TransformOutput);

pub const SYM_ABI_VERSION: &str = "rolldown_native_plugin_abi_version";
pub const SYM_TRANSFORM: &str = "rolldown_native_plugin_transform";
pub const SYM_DROP_OUTPUT: &str = "rolldown_native_plugin_drop_output";
