use std::{
  ops::{Deref, DerefMut},
  path::PathBuf,
  sync::{Arc, atomic::AtomicU32},
  time::Duration,
};

use rolldown_common::{ClientHmrUpdate, ScanMode, WatcherChangeKind};
use rolldown_error::BuildResult;
use rolldown_utils::indexmap::FxIndexSet;
use tokio::sync::Mutex;

use crate::{
  Bundler,
  dev::{build_driver_service::BuildMessage, dev_context::SharedDevContext},
  types::scan_stage_cache::ScanStageCache,
};

#[derive(Debug)]
pub struct TaskInput {
  pub changed_files: FxIndexSet<PathBuf>,
  pub require_full_rebuild: bool,
  pub generate_hmr_updates: bool,
  pub rebuild: bool,
}

impl TaskInput {
  pub fn new_initial_build_task() -> Self {
    Self {
      changed_files: FxIndexSet::default(),
      require_full_rebuild: true,
      rebuild: true,
      generate_hmr_updates: false,
    }
  }

  pub fn is_mergeable_with(&self, other: &Self) -> bool {
    match (self.require_full_rebuild, self.generate_hmr_updates, self.rebuild) {
      // Full Rebuild ONLY.
      (true, _, _) => {
        // If self is a full rebuild task:
        // - Incoming hmr update task would be meaningless, because full rebuild will bundle with latest disk files' contents.
        // - The build output will contains latest contents, it's no need to and we can't generate hmr updates for such situation.
        // - The incoming incremental rebuild task would be meaningless, because the build output will contains latest contents.
        true
      }
      // Rebuild ONLY.
      (false, false, true) => {
        // Rebuild only task can only merge with other rebuild only task.
        // If we merge a hmr update task, we'll involve files that're not intend to be involved in the hmr generation.
        other.rebuild && !other.generate_hmr_updates && !other.require_full_rebuild
      }
      // Hmr Update(include Hmr with incremental rebuild).
      (false, true, _) => {
        // Hmr update task can only merge with other Hmr update task (include hmr with incremental rebuild).
        other.generate_hmr_updates && !other.require_full_rebuild
      }
      // Noop.
      (false, false, false) => {
        eprintln!("Debug: Detect a Noop task. It should be unreachable in practice.");
        // This should be unreachable in practice.
        false
      }
    }
  }

  // You should call `is_mergeable_with` first to check if the two tasks are mergeable in business logic.
  pub fn merge_with(&mut self, other: Self) {
    self.changed_files.extend(other.changed_files);
    self.require_full_rebuild = self.require_full_rebuild || other.require_full_rebuild;
    self.generate_hmr_updates = self.generate_hmr_updates || other.generate_hmr_updates;
    self.rebuild = self.rebuild || other.rebuild;
  }
}

pub struct BundlingTask {
  pub input: TaskInput,
  pub bundler: Arc<Mutex<Bundler>>,
  pub dev_context: SharedDevContext,
  pub bundler_cache: Option<ScanStageCache>,
  pub next_hmr_patch_id: Arc<AtomicU32>,
}

impl Deref for BundlingTask {
  type Target = TaskInput;

  fn deref(&self) -> &Self::Target {
    &self.input
  }
}

impl DerefMut for BundlingTask {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.input
  }
}

impl BundlingTask {
  pub async fn run(mut self) {
    tracing::trace!("Start running bundling task: {:#?}", self.input);
    if let Err(err) = self.run_inner().await {
      // FIXME: Should handle the error properly.
      eprintln!("Build error: {err}"); // FIXME: handle this error
    }

    let mut build_state = self.dev_context.state.lock().await;
    build_state.cache = Some(self.bundler_cache.take().expect("Should never be none here"));
    if let Err(err) = build_state.try_to_idle() {
      eprintln!("TODO: should handle this error {err:#?}");
      build_state.reset_to_idle();
    }
    build_state.has_stale_build_output = !self.rebuild;
    drop(build_state);

    if self.dev_context.build_channel_tx.send(BuildMessage::BuildFinish).is_err() {
      tracing::error!("Failed to send build finish message to build channel");
      // Notice: Send error will only happen when the channel is closed. It might be closed by an error or what.
      // Handling this error is helpful to that situation.
    }
  }

  async fn run_inner(&mut self) -> BuildResult<()> {
    self.delay_to_merge_incoming_changes().await?;

    {
      let bundler = self.bundler.lock().await;
      for changed_file in &self.input.changed_files {
        bundler
          .plugin_driver
          // FIXME: use proper WatcherChangeKind for created/removed files.
          .watch_change(changed_file.to_str().unwrap(), WatcherChangeKind::Update)
          .await?;
      }
    }

    let mut has_full_reload_update = false;
    if self.generate_hmr_updates {
      self.generate_hmr_updates(&mut has_full_reload_update).await?;
    }

    // If the rebuild strategy is auto and there's a full reload update, we need to rebuild.
    if self.dev_context.options.rebuild_strategy.is_auto()
      && has_full_reload_update
      && !self.rebuild
    {
      self.rebuild = true;
    }

    if self.rebuild {
      self.rebuild().await?;
    }

    Ok(())
  }

  async fn delay_to_merge_incoming_changes(&self) -> BuildResult<()> {
    let build_delay = 0;

    let mut build_status = if build_delay > 0 {
      loop {
        tokio::time::sleep(Duration::from_millis(build_delay)).await;
        let build_status = self.dev_context.state.lock().await;

        let has_no_changes = true; // TODO: implement delay merge behavior

        if has_no_changes {
          break build_status;
        }

        drop(build_status);
      }
    } else {
      self.dev_context.state.lock().await
    };

    tracing::trace!("`BuildStatus` is in building with changed files: {:#?}", self.changed_files);
    build_status.try_to_building()?;

    drop(build_status);

    Ok(())
  }

  pub async fn generate_hmr_updates(
    &mut self,
    has_full_reload_update: &mut bool,
  ) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
    bundler.set_cache(self.bundler_cache.take().expect("Should never be none here"));
    let changed_files =
      self.changed_files.iter().map(|p| p.to_string_lossy().to_string()).collect::<Vec<_>>();

    let mut client_updates = Vec::with_capacity(self.dev_context.clients.len());
    for client in self.dev_context.clients.iter() {
      let updates = bundler
        .compute_hmr_update_for_file_changes(
          &changed_files,
          &client.executed_modules,
          Arc::clone(&self.next_hmr_patch_id),
        )
        .await?;
      for update in updates {
        if update.is_full_reload() {
          *has_full_reload_update = true;
        }
        client_updates.push(ClientHmrUpdate { client_id: client.key().to_string(), update });
      }
    }

    self.bundler_cache = Some(bundler.take_cache());
    // We had updated the cache with those changes, so we can clear the changed files.
    // This way, we won't need to update the cache again in rebuild.
    if let Some(on_hmr_updates) = self.dev_context.options.on_hmr_updates.as_ref()
      && !client_updates.is_empty()
    {
      on_hmr_updates(client_updates, changed_files);
    }

    Ok(())
  }

  async fn rebuild(&mut self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;

    if !self.require_full_rebuild {
      // We only need to pass the previous cache if it's an incremental rebuild.
      bundler.set_cache(self.bundler_cache.take().expect("Should never be none here"));
    }

    let skip_write = self.dev_context.options.skip_write;

    let scan_mode = if self.require_full_rebuild {
      ScanMode::Full
    } else {
      ScanMode::Partial(self.changed_files.iter().map(|p| p.to_string_lossy().into()).collect())
    };
    let scan_output = bundler.scan(scan_mode).await?;
    let bundle_output = if skip_write {
      bundler.bundle_generate(scan_output).await
    } else {
      bundler.bundle_write(scan_output).await
    }?;

    // Call on_output callback if provided
    if let Some(on_output) = self.dev_context.options.on_output.as_ref() {
      on_output(bundle_output);
    }

    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.changed_files
    );
    self.bundler_cache = Some(bundler.take_cache());
    Ok(())
  }
}
