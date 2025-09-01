use crate::dev::dev_context::BuildProcessFuture;

#[derive(Debug)]
pub struct BuildBuildingState {
  pub future: BuildProcessFuture,
}

#[derive(Debug)]
pub struct BuildDelayingState {
  pub future: BuildProcessFuture,
}

#[derive(Default, Debug)]
pub enum BuildState {
  #[default]
  Idle,
  Building(BuildBuildingState),
  Delaying(BuildDelayingState),
}
