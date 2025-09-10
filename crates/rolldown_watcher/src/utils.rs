use crate::{EventHandler, event::Event};
use notify::RecursiveMode;
use rolldown_error::{BuildResult, ResultExt};
use std::{path::Path, time::Instant};

#[cfg(not(target_family = "wasm"))]
pub use non_wasm::*;

pub struct NotifyEventHandlerAdapter<T: EventHandler>(pub T);

/// Adapter that wraps notify's PathsMut to implement rolldown's PathsMut trait.
/// This allows non-debounced watchers to provide batch path manipulation functionality.
pub struct NotifyPathsMutAdapter<'me>(Box<dyn notify::PathsMut + 'me>);

impl<'me> NotifyPathsMutAdapter<'me> {
  pub fn new(paths_mut: Box<dyn notify::PathsMut + 'me>) -> Self {
    Self(paths_mut)
  }
}

impl crate::PathsMut for NotifyPathsMutAdapter<'_> {
  fn add(&mut self, path: &Path, recursive_mode: RecursiveMode) -> BuildResult<()> {
    self.0.add(path, recursive_mode).map_err_to_unhandleable().map_err(Into::into)
  }

  fn remove(&mut self, path: &Path) -> BuildResult<()> {
    self.0.remove(path).map_err_to_unhandleable().map_err(Into::into)
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    self.0.commit().map_err_to_unhandleable().map_err(Into::into)
  }
}

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
