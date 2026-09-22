//! Helpers for a panic payload caught with `std::panic::catch_unwind`.
//!
//! A caught payload is a `Box<dyn Any + Send>`, and its destructor is user code:
//! it can panic again. Dropping it plainly lets that second panic unwind out of
//! the code that caught the first one. Close paths must not let that happen:
//! close clears resources "including failure" and is "terminal and idempotent"
//! (see `internal-docs/rust-bundler/implementation.md`, "`BundleHandle.close()`
//! — Design Decision"), and a `futures::Shared` close future that unwinds is
//! poisoned for every later caller. See also the rule for caught panic payloads
//! in `internal-docs/async-runtime/design.md`.

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

/// Drops a caught panic payload without letting its destructor unwind.
///
/// The payload is dropped under one `catch_unwind`. If its destructor panics, the
/// new (nested) payload is leaked with `mem::forget`, because its destructor is
/// just as untrusted. A panic never leaves this function.
pub fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

/// Like [`discard_panic_payload`], but tries one more drop before leaking.
///
/// The payload is dropped under one `catch_unwind`. If its destructor panics, the
/// nested payload is dropped under a second `catch_unwind`, so ordinary nested
/// payload state is still reclaimed. Only a payload produced by that second
/// destructor is leaked with `mem::forget`. A panic never leaves this function.
pub fn discard_panic_payload_retrying(payload: Box<dyn Any + Send>) {
  if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
    && let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload)))
  {
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

  #[test]
  fn discard_retrying_drops_twice_then_leaks() {
    let (nested_once, once_drops) = payload(1);
    discard_panic_payload_retrying(nested_once);
    assert_eq!(drops(&once_drops), 2);

    let (hostile, hostile_drops) = payload(2);
    discard_panic_payload_retrying(hostile);
    // Outer and first nested payloads are dropped; the second nested one is leaked.
    assert_eq!(drops(&hostile_drops), 2);
  }
}
