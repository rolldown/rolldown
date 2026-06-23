use arcstr::ArcStr;

#[repr(C)]
pub struct NativeStrRef {
  pub ptr: *const u8,
  pub len: usize,
}

/// Backing buffer for a `NativeStringHolder`. Two flavors so each side can pick
/// the cheapest representation:
/// - `ArcStr` on the input path: a `clone()` is an Arc count bump, not a copy.
/// - `String` on the output path: extracting it back into `HookTransformOutput::code`
///   is a move, not a copy.
enum HolderInner {
  ArcStr(ArcStr),
  String(String),
}

pub struct NativeStringHolder {
  inner: HolderInner,
  view: NativeStrRef,
  // Module id packed into the source-side holder so the napi method doesn't
  // need a separate JS-string parameter (which would cost a UTF-16↔UTF-8 round
  // trip + an extra heap allocation per call). Stored as ArcStr to keep the
  // ownership story uniform with `inner`. `id_view` is a `#[repr(C)]` borrow
  // that `id_str()` reads through; we keep `id` alive so the view stays valid.
  // Empty `ArcStr::default()` for result holders that carry no id.
  #[expect(dead_code, reason = "kept alive so id_view stays valid")]
  id: ArcStr,
  id_view: NativeStrRef,
}

impl NativeStringHolder {
  pub fn from_arcstr(s: ArcStr) -> Self {
    Self::from_arcstr_with_id(s, ArcStr::default())
  }

  pub fn from_arcstr_with_id(s: ArcStr, id: ArcStr) -> Self {
    let view = NativeStrRef { ptr: s.as_ptr(), len: s.len() };
    let id_view = NativeStrRef { ptr: id.as_ptr(), len: id.len() };
    Self { inner: HolderInner::ArcStr(s), view, id, id_view }
  }

  pub fn from_string(s: String) -> Self {
    let view = NativeStrRef { ptr: s.as_ptr(), len: s.len() };
    let id = ArcStr::default();
    let id_view = NativeStrRef { ptr: id.as_ptr(), len: id.len() };
    Self { inner: HolderInner::String(s), view, id, id_view }
  }

  pub fn as_str(&self) -> &str {
    // SAFETY: `inner` owns the buffer for the lifetime of `self`; bytes are valid UTF-8.
    unsafe {
      let bytes = std::slice::from_raw_parts(self.view.ptr, self.view.len);
      std::str::from_utf8_unchecked(bytes)
    }
  }

  pub fn id_str(&self) -> &str {
    // SAFETY: `id` ArcStr lives as long as `self`.
    unsafe {
      let bytes = std::slice::from_raw_parts(self.id_view.ptr, self.id_view.len);
      std::str::from_utf8_unchecked(bytes)
    }
  }

  pub fn into_string(self) -> String {
    match self.inner {
      HolderInner::String(s) => s,
      HolderInner::ArcStr(s) => s.as_str().to_owned(),
    }
  }

  pub fn into_raw_handle(self) -> i64 {
    Box::into_raw(Box::new(self)) as i64
  }

  /// # Safety
  /// `handle` must originate from `into_raw_handle` and must not have been
  /// reclaimed yet (no double-free).
  pub unsafe fn from_raw_handle(handle: i64) -> Self {
    *unsafe { Box::from_raw(handle as *mut Self) }
  }

  /// # Safety
  /// `handle` must originate from `into_raw_handle` and the Holder must outlive
  /// the returned borrow (don't call `from_raw_handle` while the `&str` is live).
  pub unsafe fn handle_as_str<'a>(handle: i64) -> &'a str {
    let holder: &'a Self = unsafe { &*(handle as *const Self) };
    holder.as_str()
  }

  /// # Safety
  /// Same contract as `handle_as_str`.
  pub unsafe fn handle_as_ref<'a>(handle: i64) -> &'a Self {
    unsafe { &*(handle as *const Self) }
  }
}

// SAFETY: both `ArcStr` and `String` are Send+Sync; the raw `view.ptr` aliases
// the inner buffer and is only read while the inner value (and therefore the
// buffer) is alive.
unsafe impl Send for NativeStringHolder {}
unsafe impl Sync for NativeStringHolder {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trips_a_string_including_multibyte_chars() {
    let s = "hello ✨ 世界".to_string();
    let holder = NativeStringHolder::from_string(s.clone());
    assert_eq!(holder.as_str(), &s);
    assert_eq!(holder.into_string(), s);
  }

  #[test]
  fn ptr_and_len_match_arcstr_source() {
    let arc = ArcStr::from("abc");
    let p = arc.as_ptr();
    let l = arc.len();
    let holder = NativeStringHolder::from_arcstr(arc);
    assert_eq!(holder.view.ptr, p);
    assert_eq!(holder.view.len, l);
  }

  #[test]
  fn into_string_from_string_is_a_move() {
    let holder = NativeStringHolder::from_string("unique".to_string());
    assert_eq!(holder.into_string(), "unique");
  }

  #[test]
  fn into_string_from_arcstr_copies() {
    let arc = ArcStr::from("shared");
    let holder = NativeStringHolder::from_arcstr(arc.clone());
    assert_eq!(holder.into_string(), "shared");
    assert_eq!(arc, "shared");
  }

  #[test]
  fn raw_handle_round_trip_with_string_inner() {
    let s = "round-trip ✨";
    let holder = NativeStringHolder::from_string(s.to_string());
    let handle = holder.into_raw_handle();
    unsafe {
      assert_eq!(NativeStringHolder::handle_as_str(handle), s);
      let reclaimed = NativeStringHolder::from_raw_handle(handle);
      assert_eq!(reclaimed.into_string(), s);
    }
  }

  #[test]
  fn raw_handle_round_trip_with_arcstr_inner() {
    let s = "shared-input";
    let holder = NativeStringHolder::from_arcstr(ArcStr::from(s));
    let handle = holder.into_raw_handle();
    unsafe {
      assert_eq!(NativeStringHolder::handle_as_str(handle), s);
      let reclaimed = NativeStringHolder::from_raw_handle(handle);
      drop(reclaimed);
    }
  }
}
