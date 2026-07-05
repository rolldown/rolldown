use std::{future::Future, pin::Pin, sync::Arc};

use futures::future::Shared;
use rolldown_dev_common::types::{DevCallbackError, DevCallbackResult};
use rolldown_error::{BatchedBuildDiagnostic, BuildResult};

use crate::{NormalizedDevOptions, SharedClients, type_aliases::CoordinatorSender};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;

// The future represents an ongoing `BundlingTask`
pub type BundlingFuture = Shared<PinBoxSendStaticFuture<DevCallbackResult>>;

#[derive(Debug)]
struct RetainedDevCallbackError(DevCallbackError);

impl std::fmt::Display for RetainedDevCallbackError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}

impl std::error::Error for RetainedDevCallbackError {
  fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
    Some(self.0.as_ref())
  }
}

pub fn dev_callback_result_to_build_result(result: DevCallbackResult) -> BuildResult<()> {
  result.map_err(|error| {
    BatchedBuildDiagnostic::from(anyhow::Error::new(RetainedDevCallbackError(error)))
  })
}

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  pub clients: SharedClients,
}
