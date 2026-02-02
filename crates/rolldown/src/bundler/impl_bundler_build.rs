use crate::types::bundle_output::BundleOutput;
use anyhow::Result;
use rolldown_common::{BundleMode, ScanMode};
use rolldown_error::BuildResult;

use super::bundler::Bundler;

impl Bundler {
  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    // TODO: hyf0: Bad code smell: this overlaps with `incremental_write/xxx` APIs.
    #[cfg(feature = "experimental")]
    if self.options.experimental.is_incremental_build_enabled() {
      return self.incremental_write(ScanMode::Full).await;
    }
    let bundle = self.bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
    bundle.write().await
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    #[cfg(feature = "experimental")]
    if self.options.experimental.is_incremental_build_enabled() {
      return self.incremental_generate(ScanMode::Full).await;
    }
    let bundle = self.bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
    bundle.generate().await
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  #[cfg(feature = "experimental")]
  pub async fn scan(&mut self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let bundle = self.bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
    bundle.scan().await?;
    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> Result<()> {
    self.inner_close().await
  }
}
