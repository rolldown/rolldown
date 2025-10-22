mod normalize_binding_transform_options;

pub mod minify_options_conversion;
pub mod napi_error;
pub mod normalize_binding_options;

use std::any::Any;

use napi_derive::napi;
use rolldown_tracing::try_init_tracing;

pub use normalize_binding_transform_options::normalize_binding_transform_options;

#[napi]
pub struct TraceSubscriberGuard {
  guard: Option<Box<dyn Any + Send>>,
}

#[napi]
impl TraceSubscriberGuard {
  #[napi]
  pub fn close(&mut self) {
    self.guard.take();
  }
}

#[napi]
pub fn init_trace_subscriber() -> Option<TraceSubscriberGuard> {
  let maybe_guard = try_init_tracing();
  let guard = maybe_guard?;
  Some(TraceSubscriberGuard { guard: Some(guard) })
}
