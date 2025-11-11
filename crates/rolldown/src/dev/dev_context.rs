use std::{future::Future, pin::Pin, sync::Arc};

use futures::future::Shared;

use crate::dev::{NormalizedDevOptions, SharedClients, type_aliases::CoordinatorSender};

pub type SharedDevContext = Arc<DevContext>;

pub type PinBoxSendStaticFuture<T = ()> = Pin<Box<dyn Future<Output = T> + Send + 'static>>;
pub type BuildProcessFuture = Shared<PinBoxSendStaticFuture<()>>;

pub struct DevContext {
  pub options: NormalizedDevOptions,
  pub coordinator_tx: CoordinatorSender,
  pub clients: SharedClients,
}
