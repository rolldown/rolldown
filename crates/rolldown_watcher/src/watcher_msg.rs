use crate::file_change_event::FileChangeEvent;
use crate::watch_task::WatchTaskIdx;
use tokio::sync::oneshot;

pub enum WatcherMsg {
  FileChanges { task_index: WatchTaskIdx, changes: Vec<FileChangeEvent> },
  Close(oneshot::Sender<()>),
}
