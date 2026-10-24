use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchGroupIdx;

pub enum WatcherMsg {
  FileChanges { group_index: WatchGroupIdx, changes: Vec<FileChangeEvent> },
  Close,
}
