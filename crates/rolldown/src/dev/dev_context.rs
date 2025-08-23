use std::{future::Future, path::PathBuf, pin::Pin, sync::Arc};

use futures::{
  FutureExt,
  future::{self, Shared},
};
use indexmap::IndexSet;
use tokio::sync::Mutex;

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type BuildProcessFuture = Shared<PinBoxSendStaticFuture<()>>;

pub struct BuildStatus {
  pub is_building: bool,
  pub is_debouncing: bool,
  pub changed_files: IndexSet<PathBuf>,
  pub future: BuildProcessFuture,
}
impl BuildStatus {
  pub fn is_busy(&self) -> bool {
    self.is_building || self.is_debouncing
  }
}

pub struct DevContext {
  pub status: Mutex<BuildStatus>,
}

impl DevContext {
  pub async fn ensure_current_build_finish(&self) -> () {
    let build_status = self.status.lock().await;
    if !build_status.is_busy() {
      return;
    }
    let build_process_future = build_status.future.clone();
    // Note: Inside `build_process_future`, it requires to lock `BuildStatus` to modify the status.
    // So, we need to drop the lock before we await `build_process_future`, otherwise we might get a deadlock.
    drop(build_status);
    build_process_future.await;
  }
}

impl Default for DevContext {
  fn default() -> Self {
    let future = Box::pin(future::ready(())) as PinBoxSendStaticFuture;
    Self {
      status: Mutex::new(BuildStatus {
        is_building: false,
        is_debouncing: false,
        future: future.shared(),
        changed_files: IndexSet::new(),
      }),
    }
  }
}
