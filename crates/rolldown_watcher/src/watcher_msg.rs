use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;

pub enum WatcherMsg {
  FileChanges { task_index: WatchTaskIdx, changes: Vec<FileChangeEvent> },
  Close,
}
