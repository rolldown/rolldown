use std::sync::{Arc, atomic::AtomicBool};

use anyhow::Context;
use arcstr::ArcStr;
use async_lock::Mutex;
use futures::channel::mpsc::unbounded;
use futures::{FutureExt, future::Shared};
#[cfg(feature = "testing")]
use rolldown_common::WatcherChangeKind;
use rolldown_common::{ClientHave, HmrLazyChunkOutput, HmrPatch, HmrPullResponse};
use rolldown_error::{BuildResult, ResultExt};
use rolldown_fs_watcher::{FsWatcher, FsWatcherConfig};
use rolldown_utils::{dashmap::FxDashSet, futures::try_spawn};
#[cfg(feature = "testing")]
use rustc_hash::FxHashSet;

use rolldown::{Bundler, BundlerBuilder, BundlerConfig, NormalizedBundlerOptions};

use crate::{
  BundleOutput, DevOptions,
  bundle_coordinator::BundleCoordinator,
  dev_context::{DevContext, PinBoxSendStaticFuture},
  normalize_dev_options,
  type_aliases::CoordinatorSender,
  types::{
    coordinator_msg::CoordinatorMsg, coordinator_state_snapshot::CoordinatorStateSnapshot,
    error_stage::ErrorStage,
  },
};

#[cfg(feature = "testing")]
use rolldown_utils::indexmap::FxIndexMap;
#[cfg(feature = "testing")]
use std::path::PathBuf;

type CoordinatorTaskResult = Result<(), Arc<str>>;

pub struct CoordinatorState {
  coordinator: Option<BundleCoordinator>,
  handle: Option<Shared<PinBoxSendStaticFuture<CoordinatorTaskResult>>>,
}

pub struct DevEngine {
  coordinator_sender: CoordinatorSender,
  bundler: Arc<Mutex<Bundler>>,
  /// Shared dev context, kept so out-of-coordinator entry points (e.g.
  /// `compile_lazy_entry`) can reach `options.on_additional_assets`.
  dev_context: Arc<DevContext>,
  coordinator_state: Mutex<CoordinatorState>,
  is_closed: AtomicBool,
  /// Live handle to the plugin-facing module infos — the same map plugin contexts read.
  /// Full builds clear it in place (the `Arc` identity is stable), so lock-free reads
  /// here always observe the latest build. Powers the engine-level module queries.
  module_infos: rolldown_common::SharedModuleInfoDashMap,
  module_side_effects_overrides: rolldown_common::SharedModuleSideEffectsOverrides,
}

impl DevEngine {
  pub fn new(mut config: BundlerConfig, options: DevOptions) -> BuildResult<Self> {
    // The HMR stage diffs against the scan-stage snapshot, which is only kept
    // when incremental build is on. Without it the first file change unwraps a
    // `None` snapshot and panics the worker. `prepare_build_context` already
    // forces this for `dev_mode`, but `dev()` is reachable without it.
    config.options.experimental.get_or_insert_default().incremental_build = Some(true);

    // Build the bundler from config
    let bundler = BundlerBuilder::default()
      .with_options(config.options)
      .with_plugins(config.plugins)
      .build()?;
    bundler.set_hmr_epoch(random_epoch());

    let module_infos = bundler.module_infos();
    let module_side_effects_overrides = bundler.module_side_effects_overrides();
    let cwd = bundler.options().cwd.clone();
    let bundler = Arc::new(Mutex::new(bundler));

    let normalized_options = normalize_dev_options(options);

    let (coordinator_tx, coordinator_rx) = unbounded::<CoordinatorMsg>();

    let ctx = Arc::new(DevContext {
      options: normalized_options,
      coordinator_tx: coordinator_tx.clone(),
      last_task_errored: std::sync::atomic::AtomicBool::new(false),
    });

    let watcher_config = FsWatcherConfig {
      enabled: !ctx.options.disable_watcher,
      poll_interval: ctx.options.poll_interval,
      debounce_delay: ctx.options.debounce_delay,
      compare_contents_for_polling: ctx.options.compare_contents_for_polling,
      debounce_tick_rate: ctx.options.debounce_tick_rate,
      use_polling: ctx.options.use_polling,
      use_debounce: ctx.options.use_debounce,
      ignored: ctx.options.watch_exclude.clone(),
      cwd,
    };

    let event_handler = BundleCoordinator::create_watcher_event_handler(coordinator_tx.clone());
    let watcher = FsWatcher::new(event_handler, &watcher_config)?;

    let coordinator = BundleCoordinator::new(
      Arc::clone(&bundler),
      Arc::clone(&ctx),
      coordinator_rx,
      watcher,
      watcher_config.cwd,
    );

    Ok(Self {
      coordinator_sender: coordinator_tx,
      bundler,
      dev_context: Arc::clone(&ctx),
      coordinator_state: Mutex::new(CoordinatorState {
        coordinator: Some(coordinator),
        handle: None,
      }),
      is_closed: AtomicBool::new(false),
      module_infos,
      module_side_effects_overrides,
    })
  }

  /// Same data the plugin-context `getModuleInfo` returns, readable from the engine
  /// handle at any time (no hook context needed).
  pub fn get_module_info(&self, module_id: &str) -> Option<Arc<rolldown_common::ModuleInfo>> {
    let info = self.module_infos.get(module_id).map(|entry| Arc::clone(entry.value()))?;
    Some(self.module_side_effects_overrides.apply_to_info(info))
  }

  pub fn get_module_ids(&self) -> Vec<arcstr::ArcStr> {
    self.module_infos.iter().map(|entry| entry.key().clone()).collect()
  }

  pub async fn run(&self) -> BuildResult<()> {
    let mut coordinator_state = self.coordinator_state.lock().await;

    // Spawn the coordinator
    let Some(coordinator) = coordinator_state.coordinator.take() else {
      // The coordinator is already running.
      return Ok(());
    };
    let join_handle = try_spawn(coordinator.run()).map_err(|(error, _)| {
      anyhow::anyhow!("DevEngine coordinator task submission failed: {error}")
    })?;
    let coordinator_handle = Box::pin(async move {
      join_handle
        .await
        .map_err(|error| Arc::<str>::from(format!("DevEngine coordinator task failed: {error}")))
    }) as PinBoxSendStaticFuture<CoordinatorTaskResult>;
    coordinator_state.handle = Some(coordinator_handle.shared());
    drop(coordinator_state);

    // Wait for initial build to complete. It's ok if the initial build fails, we just let it pass.
    // Recovering from errors is handled by other parts of the system.
    self.ensure_latest_bundle_output().await?;

    Ok(())
  }

  /// TODO: do we really need this as a public API? What's the use case?
  pub async fn wait_for_close(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let coordinator_state = self.coordinator_state.lock().await;
    if let Some(coordinator_handle) = coordinator_state.handle.clone() {
      if let Err(error) = coordinator_handle.await {
        return Err(anyhow::anyhow!("{error}").into());
      }
    }
    Ok(())
  }

  /// Wait for ongoing bundle to finish if there is one.
  ///
  /// If the `DevEngine` is closed while waiting, this method will return early without error.
  pub async fn wait_for_ongoing_bundle(&self) -> BuildResult<()> {
    if self.is_closed() {
      return Ok(());
    }

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    if let Err(err) =
      self.coordinator_sender.unbounded_send(CoordinatorMsg::GetState { reply: reply_sender })
    {
      if self.is_closed() {
        return Ok(());
      }
      return (Err(err))
        .map_err_to_unhandleable()
        .context("DevEngine: failed to send GetState to coordinator")?;
    }

    let Ok(status) = reply_receiver.await else {
      if self.is_closed() {
        return Ok(());
      }
      return Err(anyhow::anyhow!("DevEngine: coordinator closed before responding to GetState"))?;
    };

    if let Some(bundling_future) = status.running_future {
      bundling_future.await;
    }

    Ok(())
  }

  pub async fn get_bundle_state(&self) -> BuildResult<BundleState> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::GetState { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send GetState to coordinator within has_latest_bundle_output",
      )?;

    let status = reply_receiver.await.map_err_to_unhandleable().context(
      "DevEngine: coordinator closed before responding to GetStatus within get_bundle_state",
    )?;

    Ok(status.into())
  }

  /// Ensure there's latest bundle output available for browser loading/reloading scenarios.
  ///
  /// If the `DevEngine` is closed while waiting, this method will return early without error.
  pub async fn ensure_latest_bundle_output(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    let mut loop_count = 0u32;
    loop {
      loop_count += 1;
      if loop_count > 100 {
        debug_assert!(
          false,
          "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something is definitely wrong",
        );
        tracing::warn!(
          "[DevEngine] ensure_latest_bundle_output has looped {loop_count} times, something might be wrong",
        );
        break;
      }
      let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
      if let Err(err) = self
        .coordinator_sender
        .unbounded_send(CoordinatorMsg::EnsureLatestBundleOutput { reply: reply_sender })
      {
        if self.is_closed() {
          return Ok(());
        }
        return (Err(err))
          .map_err_to_unhandleable()
          .context("DevEngine: failed to send EnsureLatestBundleOutput to coordinator")?;
      }

      let Ok(received) = reply_receiver.await else {
        if self.is_closed() {
          return Ok(());
        }
        return Err(anyhow::anyhow!(
          "DevEngine: coordinator closed before responding to EnsureLatestBundleOutput"
        ))?;
      };

      // Wait for the build if one is running or was scheduled
      if let Some(ret) = received {
        // Either a build is ongoing, or a new build was scheduled - wait for it to complete
        ret.future.await;
        if ret.is_ensure_latest_bundle_output_future {
          break;
        }
      } else {
        break;
      }
    }

    Ok(())
  }

  pub fn trigger_full_build(&self) -> BuildResult<()> {
    self.create_error_if_closed()?;

    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::TriggerFullBuild)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send TriggerFullBuild to coordinator")?;

    Ok(())
  }

  /// Renders the patch a client asks for. `query` is the query string of the patch request
  /// (see `ClientHave::parse_query`). A query that does not parse gets a reload.
  pub async fn render_patch(&self, query: &str) -> BuildResult<HmrPullResponse<HmrPatch>> {
    self.create_error_if_closed()?;
    let mut bundler = self.bundler.lock().await;
    let num_count = bundler.hmr_session().lock().num_count();
    let have = match ClientHave::parse_query(query, num_count) {
      Ok(have) => have,
      Err(reason) => return Ok(HmrPullResponse::Reload { reason }),
    };
    bundler.render_patch(&have).await
  }

  /// Compile a lazy entry module and return compiled code.
  ///
  /// Serves a lazy route to a client. A first visit scans the route into the graph; a repeat
  /// visit renders it from the graph (see internal-docs/lazy-compilation/design.md, "Repeat
  /// Visits").
  ///
  /// # Arguments
  /// * `proxy_module_id` - The proxy module ID (with ?rolldown-lazy=1 suffix)
  /// * `query` - The query string of the lazy request, which describes what the client holds
  ///   (see `ClientHave::parse_query`). A query that does not parse gets a reload.
  ///
  /// # Errors
  /// - If the module is not a lazy proxy from an earlier build
  /// - If the compile fails
  ///
  /// # Panics
  /// - If no build has run yet
  pub async fn compile_lazy_entry(
    &self,
    proxy_module_id: String,
    query: &str,
  ) -> BuildResult<HmrPullResponse<HmrLazyChunkOutput>> {
    self.create_error_if_closed()?;
    let mut bundler = self.bundler.lock().await;
    let num_count = bundler.hmr_session().lock().num_count();
    let have = match ClientHave::parse_query(query, num_count) {
      Ok(have) => have,
      Err(reason) => return Ok(HmrPullResponse::Reload { reason }),
    };
    // Checked before the proxy is marked as fetched, so a client that must reload changes
    // nothing.
    if let Some(reason) = bundler.hmr_reload_reason(&have) {
      return Ok(HmrPullResponse::Reload { reason });
    }

    // Mark the proxy module as fetched BEFORE compilation.
    // This changes the content returned by the lazy compilation plugin's load hook
    // from the empty stub to the fetched template, which imports the real module.
    if let Some(lazy_ctx) = &bundler.lazy_compilation_context {
      lazy_ctx.mark_as_fetched(&proxy_module_id);
    }

    // Compile starting from the proxy module.
    // The plugin will return new content (fetched template) that imports the real module,
    // which triggers compilation of the actual module and its dependencies.
    let mut merged = false;
    let result = bundler.compile_lazy_entry(proxy_module_id.clone(), &have, &mut merged).await;

    // A compile that fails after its merge leaves the route in the graph, and a later visit
    // serves the route with no scan. So the assets and the watch list of this compile must
    // still reach the consumer and the coordinator.
    if result.is_ok() || merged {
      // Deliver assets emitted while compiling the lazy entry (e.g. an image
      // imported by the lazy module) before returning the code, so the consumer
      // can register/serve them before the client requests them.
      if let Some(on_additional_assets) = self.dev_context.options.on_additional_assets.as_ref() {
        let mut output = BundleOutput::default();
        bundler.file_emitter.add_additional_files(&mut output.assets, &mut output.warnings);
        if !output.assets.is_empty() {
          on_additional_assets(output);
        }
      }

      // The build output needs no rebuild for the route: pages never load a lazy route from
      // it, because every lazy `import()` calls `requestLazy`. The next build of any kind
      // takes the route from the graph. A lost HMR update is the exception: its rebuild
      // becomes the full build that reloads every client.
      // See internal-docs/lazy-compilation/design.md ("Repeat Visits").
      // The compile added the files it loaded to the watch list of the latest build.
      // A new build can start before the coordinator reads this message, and its list
      // starts empty, so send this list with the message.
      // See internal-docs/dev-engine/implementation.md §15.
      let rebuild = bundler.has_lost_hmr_update();
      self.notify_module_changed(proxy_module_id, Arc::clone(bundler.watch_files()), rebuild);
    }

    result
  }

  /// Notify the coordinator that a lazy compile ran. It always watches the compile's files,
  /// and queues a rebuild only when `rebuild` is true.
  fn notify_module_changed(
    &self,
    module_id: String,
    watch_files: Arc<FxDashSet<ArcStr>>,
    rebuild: bool,
  ) {
    let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::ModuleChanged {
      module_id,
      watch_files,
      rebuild,
    });
  }

  pub async fn close(&self) -> BuildResult<()> {
    if self.is_closed.swap(true, std::sync::atomic::Ordering::SeqCst) {
      return Ok(());
    }

    // Send close message to coordinator
    self.coordinator_sender.unbounded_send(CoordinatorMsg::Close)
      .map_err_to_unhandleable()
      .context("DevEngine: failed to send Close message to coordinator - coordinator may have already terminated")?;

    // Close the bundler (calls `closeBundle` plugin hook).
    // The bundler lock MUST be released before waiting for the coordinator below.
    // Otherwise we'd deadlock: the coordinator's Close handler waits for any running
    // bundling task to finish, and that task may need to acquire the bundler lock.
    {
      let mut bundler = self.bundler.lock().await;
      bundler.close().await?;
    }

    // Wait for coordinator to close (coordinator handles watcher cleanup)
    let coordinator_state = self.coordinator_state.lock().await;
    if let Some(coordinator_handle) = coordinator_state.handle.clone() {
      if let Err(error) = coordinator_handle.await {
        return Err(anyhow::anyhow!("{error}").into());
      }
    }
    Ok(())
  }

  pub fn is_closed(&self) -> bool {
    self.is_closed.load(std::sync::atomic::Ordering::SeqCst)
  }

  /// Returns a clone of the shared normalized bundler options
  pub async fn bundler_options(&self) -> Arc<NormalizedBundlerOptions> {
    Arc::clone(self.bundler.lock().await.options())
  }

  #[cfg(feature = "testing")]
  pub async fn ensure_task_with_changed_files(
    &self,
    changed_files: FxIndexMap<PathBuf, WatcherChangeKind>,
  ) {
    // Create synthetic file change events to simulate real file system
    // changes. The whole step goes into ONE batch: one event per message
    // would spawn one build per file, while the future awaited below only
    // covers the first, leaving the rest racing the caller's assertions.
    let events = changed_files
      .into_iter()
      .map(|(path, kind)| rolldown_fs_watcher::FsEvent::new(path, kind))
      .collect::<Vec<_>>();

    if !events.is_empty() {
      // Send WatchEvent message to coordinator (simulates real file change)
      // The coordinator will automatically schedule a build via handle_file_changes
      let _ = self.coordinator_sender.unbounded_send(CoordinatorMsg::WatchEvent(events));
    }

    // Send ScheduleBuild to ensure WatchEvent is processed (FIFO),
    // and get the build future to wait on
    let (reply_tx, reply_rx) = futures::channel::oneshot::channel();
    let _ = self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::ScheduleBuildIfStale { reply: reply_tx });

    // Wait for the build that was triggered by the file change
    if let Ok(Some(ret)) = reply_rx.await {
      ret.future.await;
    }
  }

  #[cfg(feature = "testing")]
  pub fn bundler(&self) -> Arc<Mutex<Bundler>> {
    Arc::clone(&self.bundler)
  }

  #[cfg(feature = "testing")]
  pub async fn get_watched_files(&self) -> BuildResult<FxHashSet<String>> {
    self.create_error_if_closed()?;

    let (reply_sender, reply_receiver) = futures::channel::oneshot::channel();
    self
      .coordinator_sender
      .unbounded_send(CoordinatorMsg::GetWatchedFiles { reply: reply_sender })
      .map_err_to_unhandleable()
      .context(
        "DevEngine: failed to send GetWatchedFiles to coordinator within get_watched_files",
      )?;

    let watched_files = reply_receiver.await.map_err_to_unhandleable().context(
      "DevEngine: coordinator closed before responding to GetWatchedFiles within get_watched_files",
    )?;

    Ok(watched_files)
  }

  /// Replaces the random epoch, so snapshots stay the same between runs. Call it before
  /// `run`.
  #[cfg(feature = "testing")]
  pub async fn set_hmr_epoch_for_testing(&self, epoch: u32) {
    self.bundler.lock().await.set_hmr_epoch(epoch);
  }

  fn create_error_if_closed(&self) -> BuildResult<()> {
    if self.is_closed.load(std::sync::atomic::Ordering::SeqCst) {
      Err(anyhow::anyhow!("Dev engine is closed"))?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct BundleState {
  /// True for any error state (initial or incremental).
  pub last_build_errored: bool,
  /// The stage of the last incremental failure (`Some` only in
  /// `Failed { .. }`; `None` on success and on `FullBuildFailed`). Lets
  /// the consumer force a full rebuild on access after an `Hmr`-stage
  /// failure — see `internal-docs/dev-engine/implementation.md` §12.
  pub last_error_stage: Option<ErrorStage>,
  pub has_stale_output: bool,
}

impl From<CoordinatorStateSnapshot> for BundleState {
  fn from(snapshot: CoordinatorStateSnapshot) -> Self {
    Self {
      last_build_errored: snapshot.last_build_errored,
      last_error_stage: snapshot.last_error_stage,
      has_stale_output: snapshot.has_stale_output,
    }
  }
}

/// A different number for each engine, so a client of another engine (for example one that
/// survived a dev server restart) reloads instead of sending numbers that mean other modules.
fn random_epoch() -> u32 {
  use std::hash::BuildHasher;
  let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map_or(0, |duration| duration.as_nanos());
  // `RandomState::new` gives each call new random keys, so two engines in one process differ
  // too. No process id: WASI has none, and `std::process::id` panics there.
  let hash = std::collections::hash_map::RandomState::new().hash_one(now);
  // The default session has epoch 0, so never pick it.
  u32::try_from(hash >> 32).expect("the high half of a u64 fits in a u32").max(1)
}

#[cfg(test)]
mod tests {
  #[cfg(feature = "testing")]
  use std::sync::Arc;

  use rolldown::{BundlerConfig, BundlerOptions};
  #[cfg(feature = "testing")]
  use rolldown_common::{HmrLazyChunkOutput, HmrPullResponse};
  #[cfg(feature = "testing")]
  use rolldown_error::BuildResult;

  #[cfg(feature = "testing")]
  use crate::BundleOutput;

  #[test]
  fn dev_engine_forces_incremental_build_on() {
    let engine = super::DevEngine::new(
      BundlerConfig::new(BundlerOptions::default(), vec![]),
      crate::DevOptions::default(),
    )
    .unwrap();
    assert!(
      engine.bundler.try_lock().unwrap().options().experimental.is_incremental_build_enabled()
    );
  }

  /// The query of a client that holds nothing and knows the current generation, like a tab
  /// that just loaded the latest output.
  #[cfg(feature = "testing")]
  async fn new_tab_query(engine: &super::DevEngine) -> String {
    let bundler = engine.bundler.lock().await;
    let session = bundler.hmr_session().lock();
    format!("s={}&g={}", session.epoch(), session.generation())
  }

  #[cfg(feature = "testing")]
  fn expect_payload<T>(response: HmrPullResponse<T>) -> T {
    match response {
      HmrPullResponse::Payload(payload) => payload,
      HmrPullResponse::Reload { reason } => panic!("unexpected reload: {reason}"),
    }
  }

  /// A lazy visit of `proxy` by a tab that holds nothing.
  #[cfg(feature = "testing")]
  async fn visit(engine: &super::DevEngine, proxy: &str) -> BuildResult<HmrLazyChunkOutput> {
    let query = new_tab_query(engine).await;
    engine.compile_lazy_entry(proxy.to_owned(), &query).await.map(expect_payload)
  }

  /// A lazy compile adds the files it loads to the watch list of the build it runs
  /// against. A build that starts before the coordinator handles the compile's
  /// `ModuleChanged` begins with an empty list, and no later build loads those cached
  /// modules again, so the coordinator must still end up watching them.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_files_stay_watched_when_a_build_starts_first() {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::ScanMode;

    let dir = std::env::temp_dir().join(format!("rolldown-dev-lazy-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    std::fs::write(dir.join("main.js"), "import('./lazy.js');\n").unwrap();
    std::fs::write(dir.join("lazy.js"), "export { dep } from './dep.js';\n").unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 1;\n").unwrap();
    let lazy = dir.join("lazy.js").to_string_lossy().into_owned();
    let dep = dir.join("dep.js").to_string_lossy().into_owned();

    let engine = super::DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          input: Some(vec!["./main.js".to_string().into()]),
          cwd: Some(dir.clone()),
          experimental: Some(ExperimentalOptions {
            dev_mode: Some(DevModeOptions { lazy: Some(true), ..Default::default() }),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![],
      ),
      crate::DevOptions {
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    engine.run().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    assert!(!watched.contains(&lazy) && !watched.contains(&dep), "not loaded before the compile");

    visit(&engine, &format!("{lazy}?rolldown-lazy=1")).await.unwrap();
    // The compile has sent `ModuleChanged`. The coordinator runs on a runtime worker, so it
    // can read the message before or after this build replaces the list; the message carries
    // the compile's list, so the files must end up watched in both orders.
    engine.bundler.lock().await.incremental_generate(ScanMode::Partial(vec![])).await.unwrap();
    engine.ensure_latest_bundle_output().await.unwrap();

    let watched = engine.get_watched_files().await.unwrap();
    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(watched.contains(&lazy) && watched.contains(&dep), "{watched:#?}");
  }

  /// With `RebuildStrategy::Always` one task runs the HMR stage and then a rebuild. The HMR
  /// stage adds the files it loads to the latest build's watch list. The rebuild starts a
  /// new, empty list and does not load those cached modules again, so the coordinator must
  /// still end up watching them.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn hmr_stage_files_stay_watched_when_the_same_task_rebuilds() {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::WatcherChangeKind;
    use rolldown_utils::indexmap::FxIndexMap;

    let dir =
      std::env::temp_dir().join(format!("rolldown-dev-hmr-rebuild-watch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    std::fs::write(dir.join("main.js"), "export const main = 1;\n").unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 1;\n").unwrap();
    let main = dir.join("main.js").to_string_lossy().into_owned();
    let dep = dir.join("dep.js").to_string_lossy().into_owned();

    let engine = super::DevEngine::new(
      BundlerConfig::new(
        BundlerOptions {
          input: Some(vec!["./main.js".to_string().into()]),
          cwd: Some(dir.clone()),
          experimental: Some(ExperimentalOptions {
            dev_mode: Some(DevModeOptions::default()),
            ..Default::default()
          }),
          ..Default::default()
        },
        vec![],
      ),
      crate::DevOptions {
        rebuild_strategy: Some(crate::RebuildStrategy::Always),
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    engine.run().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    assert!(watched.contains(&main) && !watched.contains(&dep), "{watched:#?}");

    std::fs::write(
      dir.join("main.js"),
      "import { dep } from './dep.js';\nexport const main = dep;\n",
    )
    .unwrap();
    engine
      .ensure_task_with_changed_files(FxIndexMap::from_iter([(
        dir.join("main.js"),
        WatcherChangeKind::Update,
      )]))
      .await;

    let watched = engine.get_watched_files().await.unwrap();
    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(watched.contains(&dep), "{watched:#?}");
  }

  /// A temp dir with `files` and a dev engine. With `RebuildStrategy::Always` every edit also
  /// gives a new output. `files` must include `main.js`, the entry.
  #[cfg(feature = "testing")]
  fn rebuild_fixture_with_files(
    name: &str,
    files: &[(&str, &str)],
    on_hmr_updates: Option<crate::OnHmrUpdatesCallback>,
    on_output: crate::OnOutputCallback,
    rebuild_strategy: crate::RebuildStrategy,
  ) -> (std::path::PathBuf, super::DevEngine) {
    use rolldown::{DevModeOptions, ExperimentalOptions};

    let dir = std::env::temp_dir().join(format!("rolldown-dev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    for (file, content) in files {
      std::fs::write(dir.join(file), content).unwrap();
    }
    let options = BundlerOptions {
      input: Some(vec!["./main.js".to_string().into()]),
      cwd: Some(dir.clone()),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions::default()),
        ..Default::default()
      }),
      ..Default::default()
    };
    let engine = super::DevEngine::new(
      BundlerConfig::new(options, vec![]),
      crate::DevOptions {
        on_hmr_updates,
        on_output: Some(on_output),
        rebuild_strategy: Some(rebuild_strategy),
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    (dir, engine)
  }

  /// The output code does not carry the generation, so an edit followed by its undo gives the
  /// same bytes again, and the browser can reuse the code it compiled for them. The generation
  /// comes with the output instead.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn an_undo_gives_back_the_same_output_bytes() {
    use rolldown_common::{Output, WatcherChangeKind};
    use rolldown_utils::indexmap::FxIndexMap;
    use std::sync::Mutex;

    let outputs = Arc::new(Mutex::new(Vec::new()));
    let on_output = {
      let outputs = Arc::clone(&outputs);
      Arc::new(move |result: BuildResult<BundleOutput>| {
        let output = result.unwrap();
        let entry = output
          .assets
          .iter()
          .find_map(|asset| match asset {
            Output::Chunk(chunk) if chunk.is_entry => Some(chunk.code.clone()),
            _ => None,
          })
          .unwrap();
        outputs.lock().unwrap().push((entry, output.hmr_generation));
      }) as crate::OnOutputCallback
    };
    let (dir, engine) = rebuild_fixture_with_files(
      "undo-same-bytes",
      &[
        ("main.js", "import { value } from './dep.js';\nconsole.log(value);\n"),
        ("dep.js", "export const value = 1;\n"),
      ],
      None,
      on_output,
      crate::RebuildStrategy::Always,
    );
    engine.run().await.unwrap();

    for content in ["export const value = 2;\n", "export const value = 1;\n"] {
      std::fs::write(dir.join("dep.js"), content).unwrap();
      engine
        .ensure_task_with_changed_files(FxIndexMap::from_iter([(
          dir.join("dep.js"),
          WatcherChangeKind::Update,
        )]))
        .await;
    }
    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let outputs = outputs.lock().unwrap();
    let generations = outputs.iter().map(|(_, generation)| *generation).collect::<Vec<_>>();
    assert_eq!(generations, [Some(0), Some(1), Some(2)]);
    assert!(outputs[0].0.contains("registerGraph"), "{}", outputs[0].0);
    assert_ne!(outputs[0].0, outputs[1].0);
    assert_eq!(outputs[0].0, outputs[2].0);
  }

  /// An update that fails after the cache merge leaves the graph with an edit that no client
  /// received. The next task reloads every client onto a full build, a request from before the
  /// reload gets `Reload`, and hot updates work again after that build.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn a_lost_update_reloads_the_clients_onto_a_full_build() {
    use rolldown_common::{ClientHave, HmrPullResponse, HmrUpdate, WatcherChangeKind};
    use rolldown_utils::indexmap::FxIndexMap;
    use std::sync::Mutex;

    let updates = Arc::new(Mutex::new(Vec::new()));
    let on_hmr_updates = {
      let updates = Arc::clone(&updates);
      Arc::new(move |result: BuildResult<(HmrUpdate, Vec<String>)>| {
        updates.lock().unwrap().push(result.map_err(|_| ()));
      }) as crate::OnHmrUpdatesCallback
    };
    let generations = Arc::new(Mutex::new(Vec::new()));
    let on_output = {
      let generations = Arc::clone(&generations);
      Arc::new(move |result: BuildResult<BundleOutput>| {
        generations.lock().unwrap().push(result.unwrap().hmr_generation);
      }) as crate::OnOutputCallback
    };
    let (dir, engine) = rebuild_fixture_with_files(
      "lost-update",
      &[
        ("main.js", "import { value } from './dep.js';\nconsole.log(value);\n"),
        ("dep.js", "export const value = 0;\n"),
      ],
      Some(on_hmr_updates),
      on_output,
      crate::RebuildStrategy::Never,
    );
    let dep = dir.join("dep.js");
    engine.set_hmr_epoch_for_testing(7).await;
    engine.run().await.unwrap();
    let edit = async |value: u32| {
      std::fs::write(&dep, format!("export const value = {value};\n")).unwrap();
      engine
        .ensure_task_with_changed_files(FxIndexMap::from_iter([(
          dep.clone(),
          WatcherChangeKind::Update,
        )]))
        .await;
      updates.lock().unwrap().pop().expect("each edit sends one update")
    };

    engine.bundler.lock().await.lose_hmr_update_for_testing();

    let (update, changed_files) = edit(2).await.unwrap();
    let HmrUpdate::FullReload { reason } = update else {
      panic!("expected a full reload, got {update:?}");
    };
    assert_eq!(reason, "an earlier hot update failed");
    assert_eq!(changed_files, [dep.to_string_lossy().into_owned()]);
    assert!(!engine.bundler.lock().await.has_lost_hmr_update());
    // The full reload is generation 1, and the full build after it carries that generation.
    assert_eq!(*generations.lock().unwrap(), [Some(0), Some(1)]);

    let from_before_the_reload = ClientHave { epoch: 7, generation: 0, ..ClientHave::default() };
    let response = engine.render_patch(&from_before_the_reload.to_query()).await.unwrap();
    assert!(matches!(response, HmrPullResponse::Reload { .. }), "{response:?}");

    // Two updates, so a successful update that leaves the flag set fails the second one.
    for (value, expected_generation) in [(3, 2), (4, 3)] {
      let (update, _) = edit(value).await.unwrap();
      assert!(
        matches!(update, HmrUpdate::Changed { generation, .. } if generation == expected_generation),
        "{update:?}"
      );
    }
    let reloaded = ClientHave { epoch: 7, generation: 3, ..ClientHave::default() };
    let response = engine.render_patch(&reloaded.to_query()).await.unwrap();
    assert!(matches!(response, HmrPullResponse::Payload(_)), "{response:?}");

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
  }

  #[cfg(feature = "testing")]
  const LAZY_FILES: &[(&str, &str)] = &[
    ("main.js", "import('./lazy.js');\n"),
    ("lazy.js", "import { dep } from './dep.js';\nexport const value = dep;\n"),
    ("dep.js", "export const dep = 1;\n"),
  ];

  /// A temp dir with `main.js` -> `import('./lazy.js')` -> `dep.js`, and a dev engine with lazy
  /// compilation and sourcemaps on. The caller runs the engine.
  #[cfg(feature = "testing")]
  fn lazy_fixture(name: &str) -> (std::path::PathBuf, super::DevEngine) {
    lazy_fixture_with_files(name, LAZY_FILES)
  }

  /// `files` must include `main.js`, the entry.
  #[cfg(feature = "testing")]
  fn lazy_fixture_with_files(
    name: &str,
    files: &[(&str, &str)],
  ) -> (std::path::PathBuf, super::DevEngine) {
    let (dir, engine, _) = lazy_fixture_reported(name, files, |_| {});
    (dir, engine)
  }

  /// What the engine reported through its callbacks.
  #[cfg(feature = "testing")]
  #[derive(Default)]
  struct Reported {
    /// The `on_output` calls that carried a build.
    builds: std::sync::atomic::AtomicUsize,
    /// Every update that `on_hmr_updates` delivered.
    hmr_updates: std::sync::Mutex<Vec<rolldown_common::HmrUpdate>>,
    /// The file names that `on_additional_assets` delivered.
    assets: std::sync::Mutex<Vec<String>>,
    /// How often the `resolveId` hook ran. See `CountResolveCalls`.
    resolve_calls: Arc<std::sync::atomic::AtomicUsize>,
  }

  #[cfg(feature = "testing")]
  impl Reported {
    fn builds(&self) -> usize {
      self.builds.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn resolve_calls(&self) -> usize {
      self.resolve_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn has_full_reload(&self) -> bool {
      self.hmr_updates.lock().unwrap().iter().any(rolldown_common::HmrUpdate::is_full_reload)
    }

    /// Whether an update announced a change of the module with this stable id.
    fn patched(&self, stable_id: &str) -> bool {
      self.hmr_updates.lock().unwrap().iter().any(|update| {
        matches!(update, rolldown_common::HmrUpdate::Changed { changed_ids, .. }
          if changed_ids.iter().any(|id| id == stable_id))
      })
    }
  }

  /// Counts the `resolveId` calls. A lazy compile resolves only when it scans: user plugins
  /// never see the proxy itself, but they resolve the call that the proxy replaced.
  #[cfg(feature = "testing")]
  #[derive(Debug)]
  struct CountResolveCalls(Arc<std::sync::atomic::AtomicUsize>);

  #[cfg(feature = "testing")]
  impl rolldown::plugin::Plugin for CountResolveCalls {
    fn name(&self) -> std::borrow::Cow<'static, str> {
      "count-resolve-calls".into()
    }

    fn register_hook_usage(&self) -> rolldown::plugin::HookUsage {
      rolldown::plugin::HookUsage::ResolveId
    }

    async fn resolve_id(
      &self,
      _ctx: &rolldown::plugin::PluginContext,
      _args: &rolldown::plugin::HookResolveIdArgs<'_>,
    ) -> rolldown::plugin::HookResolveIdReturn {
      self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      Ok(None)
    }
  }

  /// Like `lazy_fixture_with_files`. `configure` can change the bundler options, and the
  /// returned `Reported` records what the engine reported.
  #[cfg(feature = "testing")]
  fn lazy_fixture_reported(
    name: &str,
    files: &[(&str, &str)],
    configure: impl FnOnce(&mut BundlerOptions),
  ) -> (std::path::PathBuf, super::DevEngine, Arc<Reported>) {
    use rolldown::{DevModeOptions, ExperimentalOptions};
    use rolldown_common::SourceMapType;

    let dir = std::env::temp_dir().join(format!("rolldown-dev-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // The resolver returns real paths, and the temp dir can sit behind a symlink.
    let dir = dunce::canonicalize(&dir).unwrap();
    for (file, content) in files {
      std::fs::write(dir.join(file), content).unwrap();
    }

    let mut options = BundlerOptions {
      input: Some(vec!["./main.js".to_string().into()]),
      cwd: Some(dir.clone()),
      sourcemap: Some(SourceMapType::File),
      experimental: Some(ExperimentalOptions {
        dev_mode: Some(DevModeOptions { lazy: Some(true), ..Default::default() }),
        ..Default::default()
      }),
      ..Default::default()
    };
    configure(&mut options);
    let reported = Arc::new(Reported::default());
    let on_output = Arc::clone(&reported);
    let on_hmr_updates = Arc::clone(&reported);
    let on_additional_assets = Arc::clone(&reported);
    let engine = super::DevEngine::new(
      BundlerConfig::new(
        options,
        vec![rolldown::plugin::Plugin::new_shared(CountResolveCalls(Arc::clone(
          &reported.resolve_calls,
        )))],
      ),
      crate::DevOptions {
        on_output: Some(Arc::new(move |result| {
          if result.is_ok() {
            on_output.builds.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
          }
        })),
        on_hmr_updates: Some(Arc::new(move |result| {
          if let Ok((update, _)) = result {
            on_hmr_updates.hmr_updates.lock().unwrap().push(update);
          }
        })),
        on_additional_assets: Some(Arc::new(move |output| {
          on_additional_assets
            .assets
            .lock()
            .unwrap()
            .extend(output.assets.iter().map(|asset| asset.filename().to_owned()));
        })),
        watch: Some(crate::DevWatchOptions {
          enabled: Some(false),
          skip_write: true,
          ..Default::default()
        }),
        ..Default::default()
      },
    )
    .unwrap();
    (dir, engine, reported)
  }

  #[cfg(feature = "testing")]
  fn lazy_proxy_id(dir: &std::path::Path) -> String {
    format!("{}?rolldown-lazy=1", dir.join("lazy.js").to_string_lossy())
  }

  #[cfg(feature = "testing")]
  fn changed(
    dir: &std::path::Path,
    file: &str,
    kind: rolldown_common::WatcherChangeKind,
  ) -> rolldown_utils::indexmap::FxIndexMap<std::path::PathBuf, rolldown_common::WatcherChangeKind>
  {
    rolldown_utils::indexmap::FxIndexMap::from_iter([(dir.join(file), kind)])
  }

  /// A first visit merges the route into the graph and queues no build: no page loads a lazy
  /// route from the build output. The coordinator still watches the route's files, so an edit
  /// of one gives an HMR update.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_first_visit_queues_no_rebuild() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine, reported) = lazy_fixture_reported("lazy-first-visit", LAZY_FILES, |_| {});
    engine.run().await.unwrap();

    visit(&engine, &lazy_proxy_id(&dir)).await.unwrap();
    let state = engine.get_bundle_state().await.unwrap();
    let watched = engine.get_watched_files().await.unwrap();
    let builds = reported.builds();
    std::fs::write(dir.join("dep.js"), "export const dep = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "dep.js", WatcherChangeKind::Update)).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(!state.has_stale_output, "the first visit leaves the build output current");
    assert_eq!(builds, 1, "only the initial build ran");
    assert!(watched.contains(dir.join("dep.js").to_string_lossy().as_ref()), "{watched:#?}");
    assert!(reported.patched("dep.js"), "the edit of a route file gives an update");
  }

  /// A repeat visit of a fetched route does not scan: the graph already holds the route. It
  /// serves the same code and sourcemap as the first visit.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_repeat_visit_skips_the_scan_and_the_rebuild() {
    let (dir, engine, reported) = lazy_fixture_reported("lazy-repeat-visit", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    let first = visit(&engine, &proxy).await.unwrap();
    let resolves_after_first = reported.resolve_calls();
    let repeat = visit(&engine, &proxy).await.unwrap();
    let state_after_repeat = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(reported.resolve_calls(), resolves_after_first, "the repeat visit scans");
    assert!(!state_after_repeat.has_stale_output, "no visit queues a rebuild");
    assert!(first.sourcemap.is_some());
    assert_eq!((repeat.code, repeat.sourcemap), (first.code, first.sourcemap));
  }

  /// A failed first compile never merges the fetched proxy, so the next compile scans again
  /// and sees the fixed file. The proxy is read again, but it is not a lost update. The failed
  /// scan queued the proxy for a rescan, and the retry takes it out of the queue, so a third
  /// visit does not scan.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_scans_again_after_a_failed_first_compile() {
    let (dir, engine, reported) =
      lazy_fixture_reported("lazy-retry-first-compile", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    std::fs::write(dir.join("dep.js"), "export const dep = ;\n").unwrap();
    let failed = visit(&engine, &proxy).await;
    std::fs::write(dir.join("dep.js"), "export const dep = 'fixed';\n").unwrap();
    let retry = visit(&engine, &proxy).await;
    let state_after_retry = engine.get_bundle_state().await.unwrap();
    let resolves_after_retry = reported.resolve_calls();
    let third = visit(&engine, &proxy).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("the first compile hits the broken file");
    let retry = retry.unwrap();
    assert!(retry.code.contains("'fixed'") || retry.code.contains("\"fixed\""), "{}", retry.code);
    assert!(!state_after_retry.has_stale_output, "the retry queues no rebuild");
    assert!(!reported.has_full_reload());
    third.unwrap();
    assert_eq!(reported.resolve_calls(), resolves_after_retry, "the third visit scans");
  }

  /// A failed first visit of `b.js` queues its proxy for a rescan. A visit of `c.js` scans only
  /// its own proxy, so the broken `b.js` does not fail it.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visit_is_served_after_a_failed_visit_of_another_route() {
    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-visit-after-failed-route",
      &[
        ("main.js", "import('./b.js');\nimport('./c.js');\n"),
        ("b.js", "export const b = ;\n"),
        ("c.js", "export const c = 'c-value';\n"),
      ],
      |_| {},
    );
    engine.run().await.unwrap();
    let proxy = |file: &str| format!("{}?rolldown-lazy=1", dir.join(file).to_string_lossy());

    let failed = visit(&engine, &proxy("b.js")).await;
    let other = visit(&engine, &proxy("c.js")).await;
    let state = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("b.js is broken");
    let other = other.unwrap();
    assert!(other.code.contains("c-value"), "{}", other.code);
    assert!(!state.has_stale_output);
    assert!(!reported.has_full_reload());
  }

  /// A compile that fails after it merged the route keeps the route in the graph. It still
  /// delivers the route's asset and watches the route's files. The repeat visit serves the
  /// route with no rebuild.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_compile_failed_past_its_merge_keeps_the_route() {
    use std::sync::atomic::{AtomicBool, Ordering};

    use rolldown_common::{SourceMapIgnoreList, WatcherChangeKind};
    use rustc_hash::FxHashMap;

    let fail = Arc::new(AtomicBool::new(false));
    let fail_in_callback = Arc::clone(&fail);
    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-failed-past-merge",
      &[
        ("main.js", "import('./lazy.js');\n"),
        (
          "lazy.js",
          "import { dep } from './dep.js';\nimport image from './image.png';\nexport const value = [dep, image];\n",
        ),
        ("dep.js", "export const dep = 1;\n"),
        ("image.png", "not really a png"),
      ],
      move |options| {
        options.module_types =
          Some(FxHashMap::from_iter([(".png".to_string(), rolldown_common::ModuleType::Asset)]));
        // The sourcemap step runs after the merge.
        options.sourcemap_ignore_list =
          Some(SourceMapIgnoreList::new(Arc::new(move |sources, _| {
            let fail = fail_in_callback.load(Ordering::Relaxed);
            Box::pin(async move {
              if fail {
                anyhow::bail!("sourcemapIgnoreList failed");
              }
              Ok(vec![false; sources.len()])
            })
          })));
      },
    );
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    fail.store(true, Ordering::Relaxed);
    let failed = visit(&engine, &proxy).await;
    let assets_after_failure = reported.assets.lock().unwrap().clone();
    let watched = engine.get_watched_files().await.unwrap();
    fail.store(false, Ordering::Relaxed);
    let repeat = visit(&engine, &proxy).await;
    let state_after_repeat = engine.get_bundle_state().await.unwrap();
    std::fs::write(dir.join("dep.js"), "export const dep = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "dep.js", WatcherChangeKind::Update)).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    failed.expect_err("the sourcemap step fails");
    assert!(
      assets_after_failure.iter().any(|name| name.ends_with(".png")),
      "{assets_after_failure:#?}"
    );
    assert!(watched.contains(dir.join("dep.js").to_string_lossy().as_ref()), "{watched:#?}");
    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("lazy.js"), "{}", repeat.code);
    assert!(!state_after_repeat.has_stale_output, "the repeat visit queues no rebuild");
    assert_eq!(reported.builds(), 1, "only the initial build ran");
    assert!(reported.patched("dep.js"), "the edit of a route file gives an update");
  }

  /// A failed HMR scan of `other.js` queues it for a rescan, and the graph keeps its last good
  /// code. Lazy visits do not retry it: a repeat visit and a first visit of another route are
  /// both served.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visits_are_served_while_an_unrelated_file_is_broken() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine) = lazy_fixture_with_files(
      "lazy-visits-unrelated-broken",
      &[
        ("main.js", "import './other.js';\nimport('./lazy.js');\nimport('./second.js');\n"),
        ("other.js", "export const other = 1;\n"),
        ("lazy.js", "import { dep } from './dep.js';\nexport const value = dep;\n"),
        ("dep.js", "export const dep = 1;\n"),
        ("second.js", "export const second = 'second-value';\n"),
      ],
    );
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    visit(&engine, &proxy).await.unwrap();
    std::fs::write(dir.join("other.js"), "export const other = ;\n").unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "other.js", WatcherChangeKind::Update))
      .await;
    let state_after_break = engine.get_bundle_state().await.unwrap();
    let repeat = visit(&engine, &proxy).await;
    let second =
      visit(&engine, &format!("{}?rolldown-lazy=1", dir.join("second.js").to_string_lossy())).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(state_after_break.last_build_errored, "the HMR scan of the broken file fails");
    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("lazy.js"), "{}", repeat.code);
    let second = second.unwrap();
    assert!(second.code.contains("second-value"), "{}", second.code);
  }

  /// A failed HMR scan queues `other.js`. A failed visit of `b.js` adds its own proxy to the
  /// queue and keeps `other.js` there. So the next HMR task retries `other.js`, although no
  /// event announced its fix.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_failed_visit_keeps_the_other_queued_files() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine, reported) = lazy_fixture_reported(
      "lazy-failed-visit-keeps-queue",
      &[
        ("main.js", "import './other.js';\nimport './x.js';\nimport('./b.js');\n"),
        ("other.js", "export const other = 1;\n"),
        ("x.js", "export const x = 1;\n"),
        ("b.js", "export const b = ;\n"),
      ],
      |_| {},
    );
    engine.run().await.unwrap();

    std::fs::write(dir.join("other.js"), "export const other = ;\n").unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "other.js", WatcherChangeKind::Update))
      .await;
    let state_after_break = engine.get_bundle_state().await.unwrap();
    let failed =
      visit(&engine, &format!("{}?rolldown-lazy=1", dir.join("b.js").to_string_lossy())).await;
    // Fixed with no watch event: only the queue can bring `other.js` to the next HMR task.
    std::fs::write(dir.join("other.js"), "export const other = 'fixed';\n").unwrap();
    std::fs::write(dir.join("b.js"), "export const b = 1;\n").unwrap();
    std::fs::write(dir.join("x.js"), "export const x = 2;\n").unwrap();
    engine.ensure_task_with_changed_files(changed(&dir, "x.js", WatcherChangeKind::Update)).await;
    let state = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    assert!(state_after_break.last_build_errored, "the HMR scan of the broken file fails");
    failed.expect_err("b.js is broken");
    assert!(!state.last_build_errored);
    assert!(reported.patched("other.js"), "the HMR task retries the queued other.js");
  }

  /// An HMR update fails after its merge, so it is lost. A repeat visit is served from the
  /// graph, and its rebuild becomes the full build that reloads every client.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_visit_with_a_lost_update_reloads_every_client() {
    use rolldown_common::WatcherChangeKind;
    use rolldown_utils::indexmap::FxIndexMap;

    let (dir, engine, reported) =
      lazy_fixture_reported("lazy-visit-lost-update", LAZY_FILES, |_| {});
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    visit(&engine, &proxy).await.unwrap();
    // The update merges the edit, and no client receives it.
    std::fs::write(dir.join("dep.js"), "export const dep = 'lost';\n").unwrap();
    {
      let mut bundler = engine.bundler.lock().await;
      bundler
        .compute_hmr_update_for_file_changes(
          &FxIndexMap::from_iter([(
            dir.join("dep.js").to_string_lossy().into_owned(),
            WatcherChangeKind::Update,
          )]),
          false,
          false,
        )
        .await
        .unwrap();
      bundler.lose_hmr_update_for_testing();
    }
    let repeat = visit(&engine, &proxy).await;
    // Wait for the task that the visit queued.
    engine.ensure_task_with_changed_files(FxIndexMap::default()).await;
    let state_after_visit = engine.get_bundle_state().await.unwrap();

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let repeat = repeat.unwrap();
    assert!(repeat.code.contains("'lost'") || repeat.code.contains("\"lost\""), "{}", repeat.code);
    assert!(reported.has_full_reload(), "every client reloads");
    assert_eq!(reported.builds(), 2, "the initial build and the full build");
    assert!(!state_after_visit.last_build_errored && !state_after_visit.has_stale_output);
  }

  /// Deleting a fetched route's file rescans the proxy in the HMR stage, and that scan fails
  /// to resolve the file. The proxy is then queued for a rescan, so the next compile scans and
  /// reports the missing file instead of serving the deleted module from the graph.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn lazy_repeat_visit_reports_a_deleted_route_file() {
    use rolldown_common::WatcherChangeKind;

    let (dir, engine) = lazy_fixture("lazy-repeat-visit-deleted");
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);

    visit(&engine, &proxy).await.unwrap();
    std::fs::remove_file(dir.join("lazy.js")).unwrap();
    engine
      .ensure_task_with_changed_files(changed(&dir, "lazy.js", WatcherChangeKind::Delete))
      .await;
    let repeat = visit(&engine, &proxy).await;

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();

    let errors = format!("{:?}", repeat.expect_err("the deleted file must not be served"));
    assert!(errors.contains("UNRESOLVED_IMPORT") && errors.contains("lazy.js"), "{errors}");
  }

  /// The server pushes ids only. A client that cannot run its update asks for a patch and
  /// describes what it holds; the patch carries the update's modules it holds no current
  /// factory of, and the static deps below them, and tells a client that is behind what it
  /// missed.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn render_patch_carries_what_the_client_cannot_run() {
    use rolldown_common::{
      ClientHave, HmrNumSet, HmrPatch, HmrPullResponse, HmrUpdate, WatcherChangeKind,
    };
    use rolldown_utils::indexmap::FxIndexMap;

    let (dir, engine) = lazy_fixture_with_files(
      "render-patch",
      &[
        ("main.js", "import './a.js';\nimport './b.js';\n"),
        ("a.js", "export const a = 1;\nimport.meta.hot.accept();\n"),
        (
          "b.js",
          "import { u } from './utils.js';\nexport const value = u;\nimport.meta.hot.accept();\n",
        ),
        ("utils.js", "export const u = 1;\n"),
      ],
    );
    engine.set_hmr_epoch_for_testing(7).await;
    engine.run().await.unwrap();

    std::fs::write(dir.join("utils.js"), "export const u = 2;\n").unwrap();
    let update = engine
      .bundler
      .lock()
      .await
      .compute_hmr_update_for_file_changes(
        &FxIndexMap::from_iter([(
          dir.join("utils.js").to_string_lossy().into_owned(),
          WatcherChangeKind::Update,
        )]),
        false,
        false,
      )
      .await
      .unwrap();
    let HmrUpdate::Changed { changed_ids, stale_ids, generation } = update else {
      panic!("expected a changed update, got {update:?}");
    };
    assert_eq!(
      (changed_ids.as_slice(), stale_ids.as_slice(), generation),
      (&["utils.js".to_owned()][..], &["utils.js".to_owned()][..], 1)
    );

    let nums = |ids: &[&str]| {
      let bundler = engine.bundler.try_lock().unwrap();
      let session = bundler.hmr_session().lock();
      HmrNumSet::from_nums(ids.iter().map(|id| session.num_of(id).unwrap()))
    };
    let request = |generation: u32, factories: &[&str]| ClientHave {
      epoch: 7,
      generation,
      update: nums(&["b.js", "utils.js"]),
      executed: nums(&["main.js", "a.js"]),
      factories: nums(factories),
    };
    let patch = async |query: String| match engine.render_patch(&query).await.unwrap() {
      HmrPullResponse::Payload(HmrPatch { code, .. }) => code,
      HmrPullResponse::Reload { reason } => panic!("unexpected reload: {reason}"),
    };
    let prelude = "const __rolldown_payload__ = __rolldown_runtime__.beginPayload(1);";

    let code = patch(request(1, &[]).to_query()).await;
    assert!(code.starts_with(prelude), "{code}");
    assert!(code.contains(r#"registerGraph({ids:["b.js","utils.js"],localCount:2"#), "{code}");
    assert!(!code.contains("catchUp"), "{code}");
    // Payload rows carry no epoch, no generation and no stamps.
    assert!(!code.contains("epoch:") && !code.contains("gen:"), "{code}");

    // `b.js` did not change, so the factory the client holds is current.
    let code = patch(request(1, &["b.js"]).to_query()).await;
    assert!(code.contains(r#"ids:["utils.js"],localCount:1"#), "{code}");

    // The client missed generation 1, so the patch tells it what changed. `b.js` is still
    // current: it was stamped at no generation after 0.
    let code = patch(request(0, &["b.js"]).to_query()).await;
    assert!(
      code.starts_with(&format!(
        "{prelude}\n{}",
        r#"__rolldown_payload__.catchUp([{gen:1,changedIds:["utils.js"],staleIds:["utils.js"]}]);"#
      )),
      "{code}"
    );
    assert!(code.contains(r#"ids:["utils.js"],localCount:1"#), "{code}");

    // A request that names no module gets only the prelude.
    assert_eq!(patch("s=7&g=1".to_owned()).await, prelude);

    let other_engine = ClientHave { epoch: 8, ..request(1, &[]) };
    assert!(matches!(
      engine.render_patch(&other_engine.to_query()).await.unwrap(),
      HmrPullResponse::Reload { .. }
    ));
    assert!(matches!(engine.render_patch("g=1").await.unwrap(), HmrPullResponse::Reload { .. }));
    // Number 1000 was never given out.
    let unknown = format!("s=7&g=1&u={}", HmrNumSet::from_nums([1000]).encode());
    assert!(matches!(engine.render_patch(&unknown).await.unwrap(), HmrPullResponse::Reload { .. }));

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
  }

  /// A module the client holds a current factory of but never ran is not carried, but the walk
  /// goes on to its static deps: running the factory runs them too.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn render_patch_walks_through_a_held_factory_that_never_ran() {
    use rolldown_common::{ClientHave, HmrNumSet, HmrPatch, HmrPullResponse};

    let (dir, engine) = lazy_fixture_with_files(
      "render-patch-held",
      &[
        ("main.js", "import './b.js';\n"),
        ("b.js", "import './mid.js';\nimport.meta.hot.accept();\n"),
        ("mid.js", "import './leaf.js';\n"),
        ("leaf.js", "export const leaf = 1;\n"),
      ],
    );
    engine.set_hmr_epoch_for_testing(7).await;
    engine.run().await.unwrap();

    let nums = |ids: &[&str]| {
      let bundler = engine.bundler.try_lock().unwrap();
      let session = bundler.hmr_session().lock();
      HmrNumSet::from_nums(ids.iter().map(|id| session.num_of(id).unwrap()))
    };
    let have = ClientHave {
      epoch: 7,
      generation: 0,
      update: nums(&["b.js"]),
      executed: nums(&["main.js"]),
      factories: nums(&["mid.js"]),
    };
    let HmrPullResponse::Payload(HmrPatch { code, .. }) =
      engine.render_patch(&have.to_query()).await.unwrap()
    else {
      panic!("expected a patch");
    };
    assert!(code.contains(r#"__rolldown_payload__.registerFactory("b.js""#), "{code}");
    assert!(code.contains(r#"__rolldown_payload__.registerFactory("leaf.js""#), "{code}");
    assert!(!code.contains(r#"registerFactory("mid.js""#), "{code}");

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
  }

  /// The lazy endpoint answers like the patch endpoint: a client that must reload gets a
  /// reload and changes nothing, and a client that is behind gets a catch-up first.
  #[cfg(feature = "testing")]
  #[tokio::test]
  async fn compile_lazy_entry_reloads_or_catches_up_the_client() {
    use rolldown_common::{
      ClientHave, HmrLazyChunkOutput, HmrNumSet, HmrPullResponse, HmrUpdate, WatcherChangeKind,
    };
    use rolldown_utils::indexmap::FxIndexMap;

    let (dir, engine) = lazy_fixture_with_files(
      "lazy-reload",
      &[
        ("main.js", "import './utils.js';\nimport('./lazy.js');\n"),
        ("lazy.js", "import { u } from './utils.js';\nexport const value = u;\n"),
        ("utils.js", "export const u = 1;\n"),
      ],
    );
    engine.set_hmr_epoch_for_testing(7).await;
    engine.run().await.unwrap();
    let proxy = lazy_proxy_id(&dir);
    let is_fetched = async || {
      let bundler = engine.bundler.lock().await;
      bundler.lazy_compilation_context.as_ref().unwrap().fetched_entries.contains(proxy.as_str())
    };
    let have = |generation: u32| {
      let bundler = engine.bundler.try_lock().unwrap();
      let session = bundler.hmr_session().lock();
      ClientHave {
        epoch: 7,
        generation,
        executed: HmrNumSet::from_nums(
          ["main.js", "utils.js"].iter().map(|id| session.num_of(id).unwrap()),
        ),
        ..ClientHave::default()
      }
    };

    let other_engine = ClientHave { epoch: 8, ..have(0) };
    let response = engine.compile_lazy_entry(proxy.clone(), &other_engine.to_query()).await;
    assert!(matches!(response.unwrap(), HmrPullResponse::Reload { .. }));
    assert!(!is_fetched().await);

    // Generation 1 reloads every client, so generation 0 is too old.
    engine.bundler.lock().await.begin_hmr_full_reload();
    let response = engine.compile_lazy_entry(proxy.clone(), &have(0).to_query()).await;
    assert!(matches!(response.unwrap(), HmrPullResponse::Reload { .. }));
    assert!(!is_fetched().await);

    std::fs::write(dir.join("utils.js"), "export const u = 2;\n").unwrap();
    let update = engine
      .bundler
      .lock()
      .await
      .compute_hmr_update_for_file_changes(
        &FxIndexMap::from_iter([(
          dir.join("utils.js").to_string_lossy().into_owned(),
          WatcherChangeKind::Update,
        )]),
        false,
        false,
      )
      .await
      .unwrap();
    assert!(matches!(update, HmrUpdate::Changed { generation: 2, .. }), "{update:?}");

    // The client knows generation 1 and missed generation 2.
    let response = engine.compile_lazy_entry(proxy.clone(), &have(1).to_query()).await;
    let HmrPullResponse::Payload(HmrLazyChunkOutput { code, .. }) = response.unwrap() else {
      panic!("expected a lazy chunk");
    };
    assert!(
      code.starts_with(
        r#"const __rolldown_payload__ = __rolldown_runtime__.beginPayload(2);
__rolldown_payload__.catchUp([{gen:2,changedIds:["utils.js"],staleIds:["utils.js"]}]);"#
      ),
      "{code}"
    );
    assert!(code.contains(r#"__rolldown_payload__.registerFactory("lazy.js""#), "{code}");

    engine.close().await.unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
  }
}
