use crate::types::bundle_output::BundleOutput;
use anyhow::Result;
use rolldown_error::BuildResult;

use super::bundler::Bundler;

impl Bundler {
  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    let bundle = self.bundle_factory.create_bundle();
    bundle.write().await
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn generate(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    let bundle = self.bundle_factory.create_bundle();
    bundle.generate().await
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  #[cfg(feature = "experimental")]
  pub async fn scan(&mut self) -> BuildResult<()> {
    self.create_error_if_closed()?;
    let bundle = self.bundle_factory.create_bundle();
    bundle.scan().await?;

    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> Result<()> {
    self.inner_close().await
  }
}
