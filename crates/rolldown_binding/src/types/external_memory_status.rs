use std::sync::Arc;

use napi_derive::napi;

#[napi(object, object_from_js = false)]
pub struct ExternalMemoryStatus {
  pub freed: bool,
  pub reason: Option<String>,
}

/// Releases a binding handle's reference to its native data, the body of every
/// Arc-backed `drop_inner`.
///
/// Returns `freed: true` only when this was the last strong reference. The slot is
/// left `None`, so a second call reports that the memory was already freed.
pub fn release_arc<T>(slot: &mut Option<Arc<T>>) -> ExternalMemoryStatus {
  match slot.take() {
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
