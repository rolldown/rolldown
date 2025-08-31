use crate::{EventHandler, event::Event};
use std::time::Instant;

#[cfg(not(target_family = "wasm"))]
pub use non_wasm::*;

pub struct NotifyEventHandlerAdapter<T: EventHandler>(pub T);

impl<T> notify::EventHandler for NotifyEventHandlerAdapter<T>
where
  T: EventHandler,
{
  fn handle_event(&mut self, event_result: notify::Result<notify::Event>) {
    let mapped = event_result
      .map_err(|err| vec![err])
      .map(|evt| vec![Event { detail: evt, time: Instant::now() }]);

    self.0.handle_event(mapped);
  }
}

#[cfg(not(target_family = "wasm"))]
mod non_wasm {

  use crate::{EventHandler, event::Event};
  use notify_debouncer_full::{DebounceEventHandler, DebounceEventResult};

  /// This makes a Rolldown `EventHandler` to be a compatible `DebounceEventHandler`.
  /// So we could pass a Rolldown `EventHandler` to notify-debouncer-full `Debouncer`.
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
