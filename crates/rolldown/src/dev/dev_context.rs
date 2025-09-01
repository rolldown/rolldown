use std::{future::Future, pin::Pin, sync::Arc};

use futures::future::Shared;
use tokio::sync::Mutex;

use crate::dev::{NormalizedDevOptions, build_state_machine::BuildStateMachine};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type BuildProcessFuture = Shared<PinBoxSendStaticFuture<()>>;

pub struct DevContext {
  pub state: Mutex<BuildStateMachine>,
  pub options: NormalizedDevOptions,
}

impl DevContext {
  pub async fn ensure_current_build_finish(&self) -> () {
    let build_state = self.state.lock().await;
    if let Some(build_process_future) = build_state.is_busy_then_future().cloned() {
      // Note: Inside `build_process_future`, it requires to lock `BuildStatus` to modify the status.
      // So, we need to drop the lock before we await `build_process_future`, otherwise we might get a deadlock.
      drop(build_state);
      build_process_future.await;
    }
  }
}
