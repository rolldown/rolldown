use crate::file_change_event::FileChangeEvent;
use rolldown_common::WatcherChangeKind;
use rolldown_utils::indexmap::FxIndexMap;
use std::time::{Duration, Instant};

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
  Debouncing { changes: FxIndexMap<String, WatcherChangeKind>, deadline: Instant },
  /// Watcher is closing
  Closing,
  /// Watcher has closed
  Closed,
}

impl WatcherState {
  /// Handle a batch of file change events
  ///
  /// Returns the new state after processing the file changes.
  /// See "Kind Consolidation" in `meta/design/watch-mode.md` for dedup rules.
  #[must_use]
  pub fn on_file_changes(self, entries: Vec<FileChangeEvent>, debounce_duration: Duration) -> Self {
    if entries.is_empty() {
      return self;
    }
    match self {
      WatcherState::Idle => {
        let mut changes = FxIndexMap::default();
        for entry in entries {
          merge_change_kind(&mut changes, entry.path, entry.kind);
        }
        let deadline = Instant::now() + debounce_duration;
        WatcherState::Debouncing { changes, deadline }
      }
      WatcherState::Debouncing { mut changes, .. } => {
        for entry in entries {
          merge_change_kind(&mut changes, entry.path, entry.kind);
        }
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
  pub fn on_debounce_timeout(self) -> (Self, Option<FxIndexMap<String, WatcherChangeKind>>) {
    match self {
      WatcherState::Debouncing { changes, .. } => {
        if changes.is_empty() {
          (WatcherState::Idle, None)
        } else {
          (WatcherState::Idle, Some(changes))
        }
      }
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

/// Merge a new change kind into the accumulated changes for a path.
///
/// See "Kind Consolidation" in `meta/design/watch-mode.md` for the full rule table.
fn merge_change_kind(
  changes: &mut FxIndexMap<String, WatcherChangeKind>,
  path: String,
  new_kind: WatcherChangeKind,
) {
  if let Some(old_kind) = changes.get(&path).copied() {
    match (old_kind, new_kind) {
      // File was created then modified — still a creation
      (WatcherChangeKind::Create, WatcherChangeKind::Update) => {}
      // File was created then deleted — cancel out entirely
      (WatcherChangeKind::Create, WatcherChangeKind::Delete) => {
        changes.swap_remove(&path);
      }
      // File was deleted then recreated — net effect is an update
      (WatcherChangeKind::Delete, WatcherChangeKind::Create) => {
        changes.insert(path, WatcherChangeKind::Update);
      }
      // All other cases: new kind wins
      _ => {
        changes.insert(path, new_kind);
      }
    }
  } else {
    changes.insert(path, new_kind);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn default_duration() -> Duration {
    Duration::from_millis(100)
  }

  #[test]
  fn test_idle_to_debouncing_on_file_changes() {
    let state = WatcherState::Idle;
    let entries = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Update)];
    let new_state = state.on_file_changes(entries, default_duration());

    assert!(new_state.is_debouncing());
  }

  #[test]
  fn test_debouncing_accumulates_changes() {
    let state = WatcherState::Idle;
    let batch1 = vec![FileChangeEvent::new("test1.js".into(), WatcherChangeKind::Update)];
    let batch2 = vec![FileChangeEvent::new("test2.js".into(), WatcherChangeKind::Create)];

    let state = state.on_file_changes(batch1, default_duration());
    let state = state.on_file_changes(batch2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 2);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_debounce_timeout_to_idle() {
    let mut changes = FxIndexMap::default();
    changes.insert("test.js".to_string(), WatcherChangeKind::Update);
    let state = WatcherState::Debouncing { changes, deadline: Instant::now() };

    let (new_state, changes) = state.on_debounce_timeout();

    assert!(new_state.is_idle());
    assert!(changes.is_some());
    assert_eq!(changes.unwrap().len(), 1);
  }

  #[test]
  fn test_create_then_update_consolidates_to_create() {
    let state = WatcherState::Idle;
    let batch1 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Create)];
    let batch2 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Update)];

    let state = state.on_file_changes(batch1, default_duration());
    let state = state.on_file_changes(batch2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 1);
      assert_eq!(changes["test.js"], WatcherChangeKind::Create);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_create_then_delete_cancels_out() {
    let state = WatcherState::Idle;
    let batch1 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Create)];
    let batch2 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Delete)];

    let state = state.on_file_changes(batch1, default_duration());
    let state = state.on_file_changes(batch2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 0);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_delete_then_create_consolidates_to_update() {
    let state = WatcherState::Idle;
    let batch1 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Delete)];
    let batch2 = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Create)];

    let state = state.on_file_changes(batch1, default_duration());
    let state = state.on_file_changes(batch2, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 1);
      assert_eq!(changes["test.js"], WatcherChangeKind::Update);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_batch_create_then_update_within_single_call() {
    let state = WatcherState::Idle;
    let entries = vec![
      FileChangeEvent::new("test.js".into(), WatcherChangeKind::Create),
      FileChangeEvent::new("test.js".into(), WatcherChangeKind::Update),
    ];
    let state = state.on_file_changes(entries, default_duration());

    if let WatcherState::Debouncing { changes, .. } = state {
      assert_eq!(changes.len(), 1);
      assert_eq!(changes["test.js"], WatcherChangeKind::Create);
    } else {
      panic!("Expected Debouncing state");
    }
  }

  #[test]
  fn test_empty_entries_keeps_idle() {
    let state = WatcherState::Idle;
    let new_state = state.on_file_changes(vec![], default_duration());
    assert!(new_state.is_idle());
  }

  #[test]
  fn test_empty_entries_keeps_debouncing_without_deadline_reset() {
    let state = WatcherState::Idle;
    let entries = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Update)];
    let state = state.on_file_changes(entries, default_duration());
    let deadline_before = match &state {
      WatcherState::Debouncing { deadline, .. } => *deadline,
      _ => panic!("Expected Debouncing state"),
    };

    let state = state.on_file_changes(vec![], default_duration());

    match &state {
      WatcherState::Debouncing { deadline, changes, .. } => {
        assert_eq!(changes.len(), 1);
        assert_eq!(*deadline, deadline_before);
      }
      _ => panic!("Expected Debouncing state"),
    }
  }

  #[test]
  fn test_debounce_timeout_with_empty_changes_after_consolidation() {
    let state = WatcherState::Idle;
    let batch1 = vec![FileChangeEvent::new("a.js".into(), WatcherChangeKind::Create)];
    let batch2 = vec![FileChangeEvent::new("a.js".into(), WatcherChangeKind::Delete)];

    let state = state.on_file_changes(batch1, default_duration());
    let state = state.on_file_changes(batch2, default_duration());

    // Changes cancelled out — map is empty
    assert!(state.is_debouncing());

    let (new_state, changes) = state.on_debounce_timeout();
    assert!(new_state.is_idle());
    assert!(changes.is_none());
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
    let entries = vec![FileChangeEvent::new("test.js".into(), WatcherChangeKind::Update)];
    let new_state = state.on_file_changes(entries, default_duration());

    assert!(matches!(new_state, WatcherState::Closing));
  }
}
