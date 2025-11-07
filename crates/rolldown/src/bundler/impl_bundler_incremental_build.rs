use super::super::stages::scan_stage::NormalizedScanStageOutput;
use super::Bundler;
use crate::types::bundle_output::BundleOutput;
use arcstr::ArcStr;
use rolldown_common::ScanMode;
use rolldown_debug::{action, trace_action};
use rolldown_error::BuildResult;
use std::mem;

impl Bundler {
  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  pub async fn scan(
    &mut self,
    scan_mode: ScanMode<ArcStr>,
  ) -> BuildResult<NormalizedScanStageOutput> {
    self.create_error_if_closed()?;
    trace_action!(action::BuildStart { action: "BuildStart" });
    let cache = mem::take(&mut self.cache);
    let mut build = self.build_factory.create_incremental_build(cache);
    let ret = build.scan_modules(scan_mode).await;
    self.cache = build.cache;
    ret
  }

  pub(crate) async fn bundle_write(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let cache = mem::take(&mut self.cache);
    let mut build = self.build_factory.create_incremental_build(cache);
    let ret = build.bundle_write(scan_stage_output).await;
    self.cache = build.cache;
    ret
  }

  pub(crate) async fn bundle_generate(
    &mut self,
    scan_stage_output: NormalizedScanStageOutput,
  ) -> BuildResult<BundleOutput> {
    let cache = mem::take(&mut self.cache);
    let mut build = self.build_factory.create_incremental_build(cache);
    let ret = build.bundle_generate(scan_stage_output).await;
    self.cache = build.cache;
    ret
  }
}
