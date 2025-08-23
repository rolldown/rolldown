use std::{mem, path::PathBuf, sync::Arc, time::Duration};

use indexmap::IndexSet;
use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use crate::{Bundler, dev::dev_context::SharedDevContext};

pub struct BundlingTask {
  pub bundler: Arc<Mutex<Bundler>>,
  // Empty changed files mean full build instead of incremental build
  pub changed_files: IndexSet<PathBuf>,
  pub dev_data: SharedDevContext,
  pub ensure_latest_build: bool,
}

impl BundlingTask {
  pub async fn exec(mut self) {
    let build_delay = 0;

    let mut build_status = if build_delay > 0 {
      loop {
        tokio::time::sleep(Duration::from_millis(build_delay)).await;
        let mut build_status = self.dev_data.status.lock().await;
        if build_status.changed_files.is_empty() {
          break build_status;
        }
        self.changed_files.append(&mut build_status.changed_files);
      }
    } else {
      self.dev_data.status.lock().await
    };

    tracing::trace!("`BuildStatus` is in building with changed files: {:#?}", self.changed_files);
    build_status.is_building = true;
    build_status.is_debouncing = false;

    drop(build_status);

    self.build().await;
  }

  async fn build(self) {
    let build_result = self.build_inner().await;

    match build_result {
      Ok(()) => {}
      Err(_) => {
        let mut build_status = self.dev_data.status.lock().await;
        build_status.is_building = false;
      }
    }
  }

  async fn build_inner(&self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
    let changed_files = self.changed_files.iter().map(|p| p.to_string_lossy().into()).collect();
    let scan_output = bundler.scan(changed_files).await?;
    let _bundle_output = bundler.bundle_write(scan_output).await?;

    let mut build_status = loop {
      let mut build_status = self.dev_data.status.lock().await;
      if !self.ensure_latest_build || build_status.changed_files.is_empty() {
        build_status.is_building = false;
        break build_status;
      }
      let changed_files = mem::take(&mut build_status.changed_files);
      drop(build_status);
      let changed_files = changed_files.iter().map(|p| p.to_string_lossy().into()).collect();
      let scan_output = bundler.scan(changed_files).await?;
      let _bundle_output = bundler.bundle_write(scan_output).await?;
    };
    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.changed_files
    );
    build_status.is_building = false;
    Ok(())
  }
}
