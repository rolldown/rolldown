use crate::watch_task::WatchTaskIdx;
use rolldown_fs_watcher::FsEventResult;
use tokio::sync::oneshot;

pub enum WatcherMsg {
  FsEvent { task_index: WatchTaskIdx, event: FsEventResult },
  Close(oneshot::Sender<()>),
}
