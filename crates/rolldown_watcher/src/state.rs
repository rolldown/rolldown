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
/// - Debouncing + timeout → Idle (returns changes for build)
/// - Debouncing + close → Closing
/// - Closing → Closed
#[derive(Debug, Default)]
pub enum WatcherState {
  /// Waiting for file changes
  #[default]
  Idle,
  /// Collecting changes before triggering a build
  Debouncing { changes: Vec<ChangeEntry>, deadline: Instant },
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
      // Ignore changes when closing or closed
      WatcherState::Closing | WatcherState::Closed => self,
    }
  }

  /// Handle debounce timeout - transition from Debouncing to Idle
  ///
  /// Returns (new_state, changes_to_build) if transitioning to Idle,
  /// otherwise returns (self, None)
  pub fn on_debounce_timeout(self) -> (Self, Option<Vec<ChangeEntry>>) {
    match self {
      WatcherState::Debouncing { changes, .. } => (WatcherState::Idle, Some(changes)),
      other => (other, None),
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
  #[cfg(test)]
  pub fn is_idle(&self) -> bool {
    matches!(self, WatcherState::Idle)
  }

  /// Check if the watcher is debouncing
  #[cfg(test)]
  pub fn is_debouncing(&self) -> bool {
    matches!(self, WatcherState::Debouncing { .. })
  }

  /// Check if the watcher is closing or closed
  #[cfg(test)]
  #[expect(dead_code)]
  pub fn is_closing_or_closed(&self) -> bool {
    matches!(self, WatcherState::Closing | WatcherState::Closed)
  }

  /// Get the debounce deadline if in debouncing state
  #[cfg(test)]
  #[expect(dead_code)]
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
  fn test_debounce_timeout_to_idle() {
    let state = WatcherState::Debouncing {
      changes: vec![ChangeEntry::new("test.js".into(), WatcherChangeKind::Update)],
      deadline: Instant::now(),
    };

    let (new_state, changes) = state.on_debounce_timeout();

    assert!(new_state.is_idle());
    assert!(changes.is_some());
    assert_eq!(changes.unwrap().len(), 1);
  }

  #[test]
  fn test_debouncing_deduplicates_same_path() {
    let state = WatcherState::Idle;
    let entry1 = ChangeEntry::new("test.js".into(), WatcherChangeKind::Create);
    let entry2 = ChangeEntry::new("test.js".into(), WatcherChangeKind::Update);

    let state = state.on_file_change(entry1, default_duration());
    let state = state.on_file_change(entry2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 1);
      assert_eq!(changes[0].kind, WatcherChangeKind::Update);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_close_from_idle() {
    let state = WatcherState::Idle;
    let (new_state, should_close) = state.on_close();

    assert!(matches!(new_state, WatcherState::Closing));
    assert!(should_close);
  }

  #[test]
  fn test_close_from_closed_is_noop() {
    let state = WatcherState::Closed;
    let (new_state, should_close) = state.on_close();

    assert!(matches!(new_state, WatcherState::Closed));
    assert!(!should_close);
  }

  #[test]
  fn test_closing_ignores_file_changes() {
    let state = WatcherState::Closing;
    let entry = ChangeEntry::new("test.js".into(), WatcherChangeKind::Update);
    let new_state = state.on_file_change(entry, default_duration());

    assert!(matches!(new_state, WatcherState::Closing));
  }
}
