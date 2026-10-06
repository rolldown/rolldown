use derive_more::Debug;
use std::{future::Future, pin::Pin, sync::Arc};

use rolldown_error::PluginTimingsMeasurement;

type PluginTimingsInner = dyn Fn() -> Pin<Box<dyn Future<Output = anyhow::Result<PluginTimingsMeasurement>> + Send + 'static>>
  + Send
  + Sync
  + 'static;

/// How the core asks the JavaScript side what its plugin callbacks cost.
///
/// Pulled rather than pushed: the core calls this while the build is closing, so the
/// measurement it gets includes `closeBundle`. A push from the other side would have to
/// happen before `close()` and would miss it.
#[derive(Clone, Debug)]
#[debug("PluginTimingsOption::Fn(...)")]
pub struct PluginTimingsOption(Arc<PluginTimingsInner>);

impl PluginTimingsOption {
  pub fn new<F>(f: F) -> Self
  where
    F: Fn()
        -> Pin<Box<dyn Future<Output = anyhow::Result<PluginTimingsMeasurement>> + Send + 'static>>
      + Send
      + Sync
      + 'static,
  {
    Self(Arc::new(f))
  }

  pub async fn exec(&self) -> anyhow::Result<PluginTimingsMeasurement> {
    let t = self.0();
    t.await
  }
}
