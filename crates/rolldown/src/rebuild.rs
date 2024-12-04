use rolldown_common::Output;

#[derive(Debug, Default)]
pub struct RebuildManager {
  pub enabled: bool,
  pub old_assets: Vec<Output>,
}
