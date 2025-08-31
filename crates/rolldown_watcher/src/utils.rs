#[cfg(not(target_family = "wasm"))]
mod non_wasm {
  use crate::{EventHandler, event::Event};
  use notify_debouncer_full::{DebounceEventHandler, DebounceEventResult};

  pub struct DebounceEventHandlerAdapter<T: EventHandler>(pub T);

  impl<T> DebounceEventHandler for DebounceEventHandlerAdapter<T>
  where
    T: EventHandler,
  {
    fn handle_event(&mut self, event: DebounceEventResult) {
      self.0.handle_event(event.map(|events| {
        events.into_iter().map(|event| Event { detail: event.event, time: event.time }).collect()
      }));
    }
  }
}

#[cfg(not(target_family = "wasm"))]
pub use non_wasm::*;
