#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::channel::oneshot;

#[cfg(feature = "testing")]
use super::types::schedule_build_return::ScheduleBuildReturn;
use super::types::{
  coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
  ensure_latest_bundle_output_return::EnsureLatestBundleOutputReturn,
};
use rolldown_dev_common::types::DevCallbackError;
use rolldown_error::BuildResult;

pub type WatchRegistrationErrorObserverId = u64;

#[derive(Debug)]
pub struct WatchRegistrationErrorObservation {
  observer_id: Option<WatchRegistrationErrorObserverId>,
  coordinator_sender: CoordinatorSender,
}

impl WatchRegistrationErrorObservation {
  pub(crate) fn new(
    observer_id: WatchRegistrationErrorObserverId,
    coordinator_sender: CoordinatorSender,
  ) -> Self {
    Self { observer_id: Some(observer_id), coordinator_sender }
  }

  pub(crate) fn observer_id(&self) -> WatchRegistrationErrorObserverId {
    self.observer_id.expect("watch-registration observation must be active")
  }

  pub(crate) fn coordinator_sender(&self) -> &CoordinatorSender {
    &self.coordinator_sender
  }

  pub(crate) fn disarm(&mut self) -> WatchRegistrationErrorObserverId {
    self.observer_id.take().expect("watch-registration observation must be active")
  }
}

impl Drop for WatchRegistrationErrorObservation {
  fn drop(&mut self) {
    let Some(observer_id) = self.observer_id.take() else {
      return;
    };
    let _ = self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::CancelWatchRegistrationErrorObservation { observer_id });
  }
}

// GetBuildStatus message
pub type GetStateSender = oneshot::Sender<CoordinatorStateSnapshot>;

pub type BeginWatchRegistrationErrorObservationSender =
  oneshot::Sender<WatchRegistrationErrorObservation>;
pub type PreviewWatchRegistrationErrorsSender = oneshot::Sender<Option<DevCallbackError>>;

// ScheduleBuild message
#[cfg(feature = "testing")]
pub type ScheduleBuildIfStaleSender = oneshot::Sender<Option<ScheduleBuildReturn>>;

// Coordinator channel
pub type CoordinatorSender = UnboundedSender<CoordinatorMsg>;
pub type CoordinatorReceiver = UnboundedReceiver<CoordinatorMsg>;

pub type EnsureLatestBundleOutputSender = oneshot::Sender<Option<EnsureLatestBundleOutputReturn>>;
pub type CloseSender = oneshot::Sender<BuildResult<()>>;

#[cfg(feature = "testing")]
pub type GetWatchedFilesSender = oneshot::Sender<FxHashSet<String>>;
