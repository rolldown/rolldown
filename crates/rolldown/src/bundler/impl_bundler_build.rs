use crate::types::bundle_output::BundleOutput;
use rolldown_common::BundleMode;
#[cfg(feature = "experimental")]
use rolldown_common::ScanMode;
use rolldown_error::BuildResult;

use super::bundler::Bundler;

impl Bundler {
  // Contract split with `BundleHandle::close`: the HANDLE replays its terminal
  // close result by design — concurrent and late closers (and the
  // ClassicBundler failure-close aggregation) depend on observing the same
  // result. The BUNDLER delivers a terminal close failure once and must then
  // let the next build start, matching the reuse contract `Bundler` had before
  // the shared close future; otherwise one failed closeBundle hook would wedge
  // every later `write`/`generate`/`scan` behind a replayed error.
  // `create_bundle` resets the gate when it installs a fresh handle.
  async fn ensure_last_bundle_closed(&mut self) -> BuildResult<()> {
    if let Some(handle) = &self.last_bundle_handle {
      if let Err(error) = handle.close().await {
        if self.last_close_failure_delivered {
          // Already delivered for this handle; proceed so the caller can start
          // a fresh build (which installs a new handle and resets the gate).
          return Ok(());
        }
        self.last_close_failure_delivered = true;
        return Err(error.into());
      }
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
    let bundle = self.create_bundle(BundleMode::FullBuild, None)?;
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
    let bundle = self.create_bundle(BundleMode::FullBuild, None)?;
    bundle.generate().await
  }

  #[tracing::instrument(target = "devtool", level = "debug", skip_all)]
  #[cfg(feature = "experimental")]
  pub async fn scan(&mut self) -> BuildResult<()> {
    self.create_error_if_closed()?;
    self.ensure_last_bundle_closed().await?;
    let bundle = self.create_bundle(BundleMode::FullBuild, None)?;
    bundle.scan().await?;
    Ok(())
  }

  /// Close the bundler, calling the `closeBundle` plugin hook.
  #[tracing::instrument(level = "debug", skip_all)]
  pub async fn close(&mut self) -> BuildResult<()> {
    if !self.closed {
      self.closed = true;
    }
    if let Some(handle) = &self.last_bundle_handle {
      if let Err(error) = handle.close().await {
        // Same once-per-handle delivery as `ensure_last_bundle_closed`: the
        // first observer of the terminal close failure gets the error; a later
        // `close()` completes so teardown retry loops can finish (the handle
        // itself keeps replaying for callers that hold it directly).
        if !self.last_close_failure_delivered {
          self.last_close_failure_delivered = true;
          return Err(error.into());
        }
      }
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
      let bundle = self.create_bundle(BundleMode::FullBuild, None)?;
      bundle.context()
    };
    self.bundle_factory.last_bundle_handle = None;
    let result = handle.plugin_driver().close_watcher().await;
    handle.plugin_driver().clear();
    result?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use crate::{Bundler, BundlerOptions};
  use rolldown_common::InputItem;
  use rolldown_plugin::{
    HookBuildStartArgs, HookCloseBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext,
  };
  use std::{
    borrow::Cow,
    path::PathBuf,
    sync::{
      Arc,
      atomic::{AtomicUsize, Ordering},
    },
  };

  #[derive(Debug)]
  struct FailingClosePlugin {
    fail_build_start: bool,
    build_start_calls: Arc<AtomicUsize>,
    close_calls: Arc<AtomicUsize>,
  }

  impl Plugin for FailingClosePlugin {
    fn name(&self) -> Cow<'static, str> {
      "failing-close".into()
    }

    fn register_hook_usage(&self) -> HookUsage {
      HookUsage::BuildStart | HookUsage::CloseBundle
    }

    async fn build_start(
      &self,
      _ctx: &PluginContext,
      _args: &HookBuildStartArgs<'_>,
    ) -> HookNoopReturn {
      self.build_start_calls.fetch_add(1, Ordering::SeqCst);
      if self.fail_build_start {
        return Err(anyhow::anyhow!("injected buildStart failure"));
      }
      Ok(())
    }

    async fn close_bundle(
      &self,
      _ctx: &PluginContext,
      _args: Option<&HookCloseBundleArgs<'_>>,
    ) -> HookNoopReturn {
      self.close_calls.fetch_add(1, Ordering::SeqCst);
      Err(anyhow::anyhow!("injected closeBundle failure"))
    }
  }

  struct TestDir(PathBuf);

  impl TestDir {
    fn new(name: &str) -> Self {
      let path = std::env::temp_dir()
        .join(format!("rolldown-bundler-close-gate-{name}-{}", std::process::id()));
      std::fs::create_dir_all(&path).expect("create test directory");
      std::fs::write(path.join("main.js"), "export const value = 1;\n").expect("write entry");
      Self(path)
    }
  }

  impl Drop for TestDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  fn create_bundler(dir: &TestDir, plugin: Arc<FailingClosePlugin>) -> Bundler {
    Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(dir.0.clone()),
        input: Some(vec![InputItem {
          name: Some("main".to_string()),
          import: "./main.js".to_string(),
        }]),
        ..Default::default()
      },
      vec![plugin],
    )
    .expect("create bundler")
  }

  fn diagnostics_text(errors: &rolldown_error::BatchedBuildDiagnostic) -> String {
    errors.iter().map(ToString::to_string).collect::<Vec<_>>().join(" | ")
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn close_failure_after_successful_build_is_delivered_once_then_builds_resume() {
    let build_start_calls = Arc::new(AtomicUsize::new(0));
    let close_calls = Arc::new(AtomicUsize::new(0));
    let dir = TestDir::new("successful-build");
    let mut bundler = create_bundler(
      &dir,
      Arc::new(FailingClosePlugin {
        fail_build_start: false,
        build_start_calls: Arc::clone(&build_start_calls),
        close_calls: Arc::clone(&close_calls),
      }),
    );

    // Build 1 succeeds; nothing has been closed yet.
    bundler.generate().await.map_err(|e| diagnostics_text(&e)).expect("first build succeeds");
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 0);

    // Build 2: closing the previous bundle runs the failing closeBundle hook.
    // The terminal failure is delivered to this caller, and no build starts.
    let Err(errors) = bundler.generate().await else {
      panic!("second build must deliver the closeBundle failure");
    };
    assert!(
      diagnostics_text(&errors).contains("injected closeBundle failure"),
      "unexpected second-build errors: {}",
      diagnostics_text(&errors)
    );
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 1, "no new build may start");
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);

    // Build 3: the failure was already delivered; the bundler must start a
    // fresh build instead of replaying the stored close failure forever.
    bundler
      .generate()
      .await
      .map_err(|e| diagnostics_text(&e))
      .expect("third build must run after the close failure was delivered once");
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 2, "third build must run buildStart");
    assert_eq!(close_calls.load(Ordering::SeqCst), 1, "closeBundle hook must run exactly once");
  }

  #[tokio::test(flavor = "multi_thread")]
  async fn failed_scan_close_failure_is_delivered_once_then_rebuilds() {
    let build_start_calls = Arc::new(AtomicUsize::new(0));
    let close_calls = Arc::new(AtomicUsize::new(0));
    let dir = TestDir::new("failed-scan");
    let mut bundler = create_bundler(
      &dir,
      Arc::new(FailingClosePlugin {
        fail_build_start: true,
        build_start_calls: Arc::clone(&build_start_calls),
        close_calls: Arc::clone(&close_calls),
      }),
    );

    // Build 1: the scan fails; the failed-scan close runs (and fails), but the
    // caller must see only the scan diagnostics.
    let Err(errors) = bundler.generate().await else { panic!("first build must fail in scan") };
    let text = diagnostics_text(&errors);
    assert!(text.contains("injected buildStart failure"), "unexpected first-build errors: {text}");
    assert!(!text.contains("closeBundle"), "close result must be dropped, got: {text}");
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 1);
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);

    // Build 2: the stored close failure is delivered once; no build starts.
    let Err(errors) = bundler.generate().await else {
      panic!("second build must deliver the closeBundle failure");
    };
    let text = diagnostics_text(&errors);
    assert!(
      text.contains("injected closeBundle failure"),
      "unexpected second-build errors: {text}"
    );
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 1, "no new build may start");
    assert_eq!(close_calls.load(Ordering::SeqCst), 1);

    // Build 3: the bundler rebuilds — buildStart runs again and the caller sees
    // the new scan diagnostics, not a replayed close failure.
    let Err(errors) = bundler.generate().await else { panic!("third build must fail in scan") };
    let text = diagnostics_text(&errors);
    assert!(text.contains("injected buildStart failure"), "unexpected third-build errors: {text}");
    assert!(!text.contains("closeBundle"), "third build must not replay the close failure: {text}");
    assert_eq!(build_start_calls.load(Ordering::SeqCst), 2, "third build must run buildStart");
    assert_eq!(close_calls.load(Ordering::SeqCst), 2, "the fresh bundle's close runs once more");
  }
}
