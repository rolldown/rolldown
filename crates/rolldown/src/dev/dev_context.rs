use std::{future::Future, pin::Pin, sync::Arc};

use futures::future::Shared;
use tokio::sync::Mutex;

use crate::dev::build_state_machine::BuildStateMachine;

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type BuildProcessFuture = Shared<PinBoxSendStaticFuture<()>>;

pub struct DevContext {
  pub status: Mutex<BuildStateMachine>,
}

impl DevContext {
  pub async fn ensure_current_build_finish(&self) -> () {
    let build_status = self.status.lock().await;
    if let Some(build_process_future) = build_status.is_busy_then_future().cloned() {
      // Note: Inside `build_process_future`, it requires to lock `BuildStatus` to modify the status.
      // So, we need to drop the lock before we await `build_process_future`, otherwise we might get a deadlock.
      drop(build_status);
      build_process_future.await;
    }
  }
}

impl Default for DevContext {
  fn default() -> Self {
    Self { status: Mutex::new(BuildStateMachine::default()) }
  }
}
