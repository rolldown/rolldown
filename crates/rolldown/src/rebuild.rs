use rolldown_common::Output;

use crate::stages::link_stage::LinkStageOutput;

#[derive(Debug, Default)]
pub struct RebuildManager {
  pub enabled: bool,
  pub old_link_stage_output: Option<LinkStageOutput>,
  pub old_assets: Vec<Output>,
}
