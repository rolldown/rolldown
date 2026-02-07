use arcstr::ArcStr;
use rolldown_common::WatcherChangeKind;
use std::time::{Duration, Instant};

/// A change entry representing a file change event
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ChangeEntry {
  pub path: ArcStr,
  pub kind: WatcherChangeKind,
}

impl ChangeEntry {
  pub fn new(path: ArcStr, kind: WatcherChangeKind) -> Self {
    Self { path, kind }
  }
}

/// The state machine for the watcher
///
/// State transitions:
/// - Idle + file_change → Debouncing
/// - Idle + close → Closing
/// - Debouncing + file_change → Debouncing (extend deadline, add to changes)
/// - Debouncing + timeout → Building
/// - Debouncing + close → Closing
/// - Building (handled synchronously, pending changes queued)
/// - Building complete + no pending → Idle
/// - Building complete + pending → Debouncing
/// - Closing → Closed
#[derive(Debug, Default)]
pub enum WatcherState {
  /// Waiting for file changes
  #[default]
  Idle,
  /// Collecting changes before triggering a build
  Debouncing { changes: Vec<ChangeEntry>, deadline: Instant },
  /// Build is in progress
  Building { pending_changes: Vec<ChangeEntry> },
  /// Watcher is closing
  Closing,
  /// Watcher has closed
  Closed,
}

impl WatcherState {
  /// Handle a file change event
  ///
  /// Returns the new state after processing the file change
  #[must_use]
  pub fn on_file_change(self, entry: ChangeEntry, debounce_duration: Duration) -> Self {
    match self {
      WatcherState::Idle => {
        let deadline = Instant::now() + debounce_duration;
        WatcherState::Debouncing { changes: vec![entry], deadline }
      }
      WatcherState::Debouncing { mut changes, .. } => {
        // Check if we already have a change for this path
        if let Some(existing) = changes.iter_mut().find(|c| c.path == entry.path) {
          existing.kind = entry.kind;
        } else {
          changes.push(entry);
        }
        // Reset the deadline
        let deadline = Instant::now() + debounce_duration;
        WatcherState::Debouncing { changes, deadline }
      }
      WatcherState::Building { mut pending_changes } => {
        // Queue the change for after the build completes
        if let Some(existing) = pending_changes.iter_mut().find(|c| c.path == entry.path) {
          existing.kind = entry.kind;
        } else {
          pending_changes.push(entry);
        }
        WatcherState::Building { pending_changes }
      }
      // Ignore changes when closing or closed
      WatcherState::Closing | WatcherState::Closed => self,
    }
  }

  /// Handle debounce timeout - transition from Debouncing to Building
  ///
  /// Returns (new_state, changes_to_build) if transitioning to Building,
  /// otherwise returns (self, None)
  pub fn on_debounce_timeout(self) -> (Self, Option<Vec<ChangeEntry>>) {
    match self {
      WatcherState::Debouncing { changes, .. } => {
        (WatcherState::Building { pending_changes: Vec::new() }, Some(changes))
      }
      other => (other, None),
    }
  }

  /// Handle build completion
  ///
  /// Returns the new state after build completes
  #[must_use]
  pub fn on_build_complete(self, debounce_duration: Duration) -> Self {
    match self {
      WatcherState::Building { pending_changes } => {
        if pending_changes.is_empty() {
          WatcherState::Idle
        } else {
          // Start debouncing with pending changes
          let deadline = Instant::now() + debounce_duration;
          WatcherState::Debouncing { changes: pending_changes, deadline }
        }
      }
      other => other,
    }
  }

  /// Handle close request
  ///
  /// Returns true if we should proceed with closing
  pub fn on_close(self) -> (Self, bool) {
    match self {
      WatcherState::Closed => (WatcherState::Closed, false),
      _ => (WatcherState::Closing, true),
    }
  }

  /// Transition to closed state
  #[must_use]
  pub fn to_closed(self) -> Self {
    WatcherState::Closed
  }

  /// Check if the watcher is idle
  pub fn is_idle(&self) -> bool {
    matches!(self, WatcherState::Idle)
  }

  /// Check if the watcher is debouncing
  pub fn is_debouncing(&self) -> bool {
    matches!(self, WatcherState::Debouncing { .. })
  }

  /// Check if the watcher is building
  pub fn is_building(&self) -> bool {
    matches!(self, WatcherState::Building { .. })
  }

  /// Check if the watcher is closing or closed
  pub fn is_closing_or_closed(&self) -> bool {
    matches!(self, WatcherState::Closing | WatcherState::Closed)
  }

  /// Get the debounce deadline if in debouncing state
  pub fn debounce_deadline(&self) -> Option<Instant> {
    match self {
      WatcherState::Debouncing { deadline, .. } => Some(*deadline),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn default_duration() -> Duration {
    Duration::from_millis(100)
  }

  #[test]
  fn test_idle_to_debouncing_on_file_change() {
    let state = WatcherState::Idle;
    let entry = ChangeEntry::new("test.js".into(), WatcherChangeKind::Update);
    let new_state = state.on_file_change(entry, default_duration());

    assert!(new_state.is_debouncing());
  }

  #[test]
  fn test_debouncing_accumulates_changes() {
    let state = WatcherState::Idle;
    let entry1 = ChangeEntry::new("test1.js".into(), WatcherChangeKind::Update);
    let entry2 = ChangeEntry::new("test2.js".into(), WatcherChangeKind::Create);

    let state = state.on_file_change(entry1, default_duration());
    let state = state.on_file_change(entry2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 2);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_debounce_timeout_to_building() {
    let state = WatcherState::Debouncing {
      changes: vec![ChangeEntry::new("test.js".into(), WatcherChangeKind::Update)],
      deadline: Instant::now(),
    };

    let (new_state, changes) = state.on_debounce_timeout();

    assert!(new_state.is_building());
    assert!(changes.is_some());
    assert_eq!(changes.unwrap().len(), 1);
  }

  #[test]
  fn test_build_complete_to_idle() {
    let state = WatcherState::Building { pending_changes: Vec::new() };
    let new_state = state.on_build_complete(default_duration());

    assert!(new_state.is_idle());
  }

  #[test]
  fn test_build_complete_with_pending_to_debouncing() {
    let state = WatcherState::Building {
      pending_changes: vec![ChangeEntry::new("test.js".into(), WatcherChangeKind::Update)],
    };
    let new_state = state.on_build_complete(default_duration());

    assert!(new_state.is_debouncing());
  }

  #[test]
  fn test_building_queues_changes() {
    let state = WatcherState::Building { pending_changes: Vec::new() };
    let entry = ChangeEntry::new("test.js".into(), WatcherChangeKind::Update);
    let new_state = state.on_file_change(entry, default_duration());

    if let WatcherState::Building { pending_changes } = new_state {
      assert_eq!(pending_changes.len(), 1);
    } else {
      panic!("Expected Building state");
    }
  }
}
