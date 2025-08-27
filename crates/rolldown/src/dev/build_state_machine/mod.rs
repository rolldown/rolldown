pub mod build_state;
use build_state::{BuildBuildingState, BuildDelayingState, BuildState};
use rolldown_error::BuildResult;

use crate::dev::dev_context::BuildProcessFuture;
use indexmap::IndexSet;
use std::path::PathBuf;

#[derive(Default, Debug)]
pub struct BuildStateMachine<State = BuildState> {
  pub changed_files: IndexSet<PathBuf>,
  pub state: State,
}

impl BuildStateMachine<BuildState> {
  pub fn is_busy(&self) -> bool {
    matches!(self.state, BuildState::Building { .. } | BuildState::Delaying { .. })
  }

  pub fn is_busy_then_future(&self) -> Option<&BuildProcessFuture> {
    match &self.state {
      BuildState::Building(inner) => Some(&inner.future),
      BuildState::Delaying(inner) => Some(&inner.future),
      BuildState::Idle => None,
    }
  }

  pub fn is_building(&self) -> bool {
    matches!(self.state, BuildState::Building { .. })
  }

  pub fn is_delaying(&self) -> bool {
    matches!(self.state, BuildState::Delaying { .. })
  }

  pub fn try_to_delaying(&mut self, future: BuildProcessFuture) -> BuildResult<()> {
    match &self.state {
      BuildState::Idle => {
        self.state = BuildState::Delaying(BuildDelayingState { future });
        Ok(())
      }
      BuildState::Building(_) => {
        Err(anyhow::format_err!("Illegal state switching to `Delaying` state from `Building`."))?
      }
      BuildState::Delaying(_) => {
        Err(anyhow::format_err!("Illegal state switching to `Delaying` state from `Delaying`."))?
      }
    }
  }

  pub fn try_to_building(&mut self) -> BuildResult<()> {
    match &self.state {
      BuildState::Idle => {
        Err(anyhow::format_err!("Illegal state switching to `Building` state from `Idle`."))?
      }
      BuildState::Building(_) => {
        Err(anyhow::format_err!("Illegal state switching to `Building` state from `Building`."))?
      }
      BuildState::Delaying(inner) => {
        let future = inner.future.clone();
        self.state = BuildState::Building(BuildBuildingState { future });
        Ok(())
      }
    }
  }

  pub fn try_to_idle(&mut self) -> BuildResult<()> {
    match &self.state {
      BuildState::Idle => {
        Err(anyhow::format_err!("Illegal state switching to `Idle` state from `Idle`."))?
      }
      BuildState::Delaying(_) => {
        Err(anyhow::format_err!("Illegal state switching to `Idle` state from `Delaying`."))?
      }
      BuildState::Building(_) => {
        self.state = BuildState::Idle;
        Ok(())
      }
    }
  }
}
