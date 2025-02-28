use std::borrow::Cow;

pub fn pretty_type_name<T: ?Sized>() -> Cow<'static, str> {
  let type_name = std::any::type_name::<T>();
  remove_module_paths(type_name)
}

/// Removes module path prefixes from a type name
/// For example, converts "std::option::Option<std::string::String>" to "Option<String>"
fn remove_module_paths(type_name: &str) -> Cow<'_, str> {
  // Quick return if there are no module paths
  if memchr::memmem::find(type_name.as_bytes(), b"::").is_none() {
    return Cow::Borrowed(type_name);
  }

  // Calculate necessary buffer size (pessimistic - assumes we keep everything)
  let mut result = String::with_capacity(type_name.len());

  // Track our current position as we scan through the string
  let mut start_pos = 0;

  // Process the entire string
  while start_pos < type_name.len() {
    // Find the next ::
    if let Some(colon_pos) = type_name[start_pos..].find("::") {
      let abs_colon_pos = start_pos + colon_pos;

      // Find the start of the module name
      let mut module_start = abs_colon_pos;
      while module_start > 0 && is_word_char(type_name.as_bytes()[module_start - 1]) {
        module_start -= 1;
      }

      // Add any text before the module name
      if module_start > start_pos {
        result.push_str(&type_name[start_pos..module_start]);
      }

      // Skip the module name and the :: part
      start_pos = abs_colon_pos + 2;
    } else {
      // No more :: found, add the rest of the string
      result.push_str(&type_name[start_pos..]);
      break;
    }
  }

  Cow::Owned(result)
}

/// Checks if a byte is a word character (alphanumeric or underscore)
#[inline]
fn is_word_char(b: u8) -> bool {
  b.is_ascii_alphanumeric() || b == b'_'
}

#[test]
fn test_pretty_type_name() {
  struct Custom;
  assert_eq!(pretty_type_name::<std::option::Option<std::string::String>>(), "Option<String>");
  assert_eq!(pretty_type_name::<std::option::Option<Custom>>(), "Option<Custom>");
}

#[test]
fn test_remove_module_paths() {
  assert_eq!(
    remove_module_paths(
      "napi::threadsafe_function::ThreadsafeFunction<rolldown_binding::types::binding_rendered_chunk::RenderedChunk, napi::bindgen_runtime::js_values::either::Either<napi::bindgen_runtime::js_values::either::Either<napi::bindgen_runtime::js_values::promise::Promise<core::option::Option<alloc::string::String>>, core::option::Option<alloc::string::String>>, napi::threadsafe_function::UnknownReturnValue>, false>"
    ),
    "ThreadsafeFunction<RenderedChunk, Either<Either<Promise<Option<String>>, Option<String>>, UnknownReturnValue>, false>"
  );
}
