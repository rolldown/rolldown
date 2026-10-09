//! Helpers for a panic payload caught with `std::panic::catch_unwind`.
//!
//! A caught payload's destructor is user code and can panic again; dropping it
//! plainly lets that panic unwind out of the code that caught the first one.
//! Inside a napi callback it escapes the callback. On a close path it skips the
//! cleanup after the caught hook; `BundleHandle::close` marks itself closed
//! first, so no later call runs that cleanup. See
//! `internal-docs/async-runtime/design.md`, "Caught panic payloads are user code".

use std::{
  any::Any,
  panic::{AssertUnwindSafe, catch_unwind},
};

/// Returns the text of a panic payload.
///
/// `panic!("...")` payloads are `&'static str` or `String`; any other payload type
/// yields `"non-string panic payload"`.
pub fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
  if let Some(message) = payload.downcast_ref::<String>() {
    message.clone()
  } else if let Some(message) = payload.downcast_ref::<&str>() {
    (*message).to_string()
  } else {
    "non-string panic payload".to_string()
  }
}

/// Drops a caught panic payload without letting its destructor unwind. A nested
/// payload from that destructor is leaked, since its destructor is just as untrusted.
pub fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A payload whose destructor panics `depth` more times, each time with a
  /// payload of the same kind one level shallower.
  struct PanicOnDrop {
    depth: usize,
    drops: std::sync::Arc<std::sync::atomic::AtomicUsize>,
  }

  impl Drop for PanicOnDrop {
    fn drop(&mut self) {
      self.drops.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      if self.depth > 0 {
        std::panic::resume_unwind(Box::new(PanicOnDrop {
          depth: self.depth - 1,
          drops: std::sync::Arc::clone(&self.drops),
        }));
      }
    }
  }

  fn payload(
    depth: usize,
  ) -> (Box<dyn Any + Send>, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    (Box::new(PanicOnDrop { depth, drops: std::sync::Arc::clone(&drops) }), drops)
  }

  fn drops(counter: &std::sync::atomic::AtomicUsize) -> usize {
    counter.load(std::sync::atomic::Ordering::SeqCst)
  }

  #[test]
  fn message_reads_string_and_str_payloads() {
    assert_eq!(panic_payload_message(&String::from("owned")), "owned");
    assert_eq!(panic_payload_message(&"static"), "static");
    assert_eq!(panic_payload_message(&42_u32), "non-string panic payload");
  }

  #[test]
  fn discard_drops_once_then_leaks() {
    let (plain, plain_drops) = payload(0);
    discard_panic_payload(plain);
    assert_eq!(drops(&plain_drops), 1);

    let (hostile, hostile_drops) = payload(2);
    discard_panic_payload(hostile);
    // The outer payload is dropped; the nested one is leaked, not dropped.
    assert_eq!(drops(&hostile_drops), 1);
  }
}
