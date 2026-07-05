use crate::types::bundle_output::BundleOutput;
use rolldown_common::BundleMode;
#[cfg(feature = "experimental")]
use rolldown_common::ScanMode;
use rolldown_error::BuildResult;

use super::bundler::Bundler;

impl Bundler {
  // `&mut self` is required (not `&self`) to keep the returned future `Send`,
  // since `Bundler` is `!Sync` and `&Bundler` across an `.await` would be `!Send`.
  #[expect(clippy::needless_pass_by_ref_mut)]
  async fn ensure_last_bundle_closed(&mut self) -> BuildResult<()> {
    if let Some(handle) = &self.last_bundle_handle {
      handle.close().await?;
    }
    Ok(())
  }

  #[tracing::instrument(level = "debug", skip_all, parent = &self.session.span)]
  pub async fn write(&mut self) -> BuildResult<BundleOutput> {
    self.create_error_if_closed()?;
    self.ensure_last_bundle_closed().await?;
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
    self.ensure_last_bundle_closed().await?;
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
    self.ensure_last_bundle_closed().await?;
    let bundle = self.bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
    bundle.scan().await?;
    Ok(())
  }

  /// Close the bundler, calling the `closeBundle` plugin hook.
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> BuildResult<()> {
    if self.closed {
      return Ok(());
    }
    self.closed = true;
    if let Some(handle) = &self.last_bundle_handle {
      handle.close().await?;
    }
    Ok(())
  }

  /// Call the watch-session `closeWatcher` hook even when the watcher closes
  /// before its first build creates a bundle/plugin driver.
  // See internal-docs/watch-mode/implementation.md.
  pub async fn close_watcher(&mut self) -> BuildResult<()> {
    if let Some(handle) = self.last_bundle_handle.clone() {
      handle.plugin_driver().close_watcher().await?;
      return Ok(());
    }

    // Plugin drivers are normally created as part of `create_bundle`. An
    // immediate watcher close still owes plugins `closeWatcher`, but must not
    // manufacture a `closeBundle` lifecycle for a build that never started.
    let handle = {
      let bundle = self.bundle_factory.create_bundle(BundleMode::FullBuild, None)?;
      bundle.context()
    };
    self.bundle_factory.last_bundle_handle = None;
    let result = handle.plugin_driver().close_watcher().await;
    handle.plugin_driver().clear();
    result?;
    Ok(())
  }
}
