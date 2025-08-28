use std::{mem, path::PathBuf, sync::Arc, time::Duration};

use indexmap::IndexSet;
use rolldown_error::BuildResult;
use tokio::sync::Mutex;

use crate::{Bundler, dev::dev_context::SharedDevContext};

pub struct BundlingTask {
  pub bundler: Arc<Mutex<Bundler>>,
  pub changed_files: IndexSet<PathBuf>,
  pub require_full_rebuild: bool,
  pub dev_data: SharedDevContext,
  pub ensure_latest_build: bool,
}

impl BundlingTask {
  pub async fn exec(mut self) {
    let build_delay = 0;

    let mut build_status = if build_delay > 0 {
      loop {
        tokio::time::sleep(Duration::from_millis(build_delay)).await;
        let mut build_status = self.dev_data.state.lock().await;
        if build_status.changed_files.is_empty() && !build_status.require_full_rebuild {
          break build_status;
        }
        if build_status.require_full_rebuild {
          self.require_full_rebuild = true;
        } else {
          self.changed_files.append(&mut build_status.changed_files);
        }
        drop(build_status);
      }
    } else {
      self.dev_data.state.lock().await
    };

    tracing::trace!("`BuildStatus` is in building with changed files: {:#?}", self.changed_files);
    build_status.try_to_building().expect("FIXME: Should not unwrap here");

    drop(build_status);

    self.build().await;
  }

  async fn build(self) {
    let build_result = self.build_inner().await;

    match build_result {
      Ok(()) => {}
      Err(_) => {
        let mut build_status = self.dev_data.state.lock().await;
        build_status.try_to_idle().expect("FIXME: Should not unwrap here");
      }
    }
  }

  async fn build_inner(&self) -> BuildResult<()> {
    let mut bundler = self.bundler.lock().await;
    let changed_files = if self.require_full_rebuild {
      vec![]
    } else {
      self.changed_files.iter().map(|p| p.to_string_lossy().into()).collect()
    };
    let scan_output = bundler.scan(changed_files).await?;
    let _bundle_output = bundler.bundle_write(scan_output).await?;

    let mut build_status = loop {
      let mut build_status = self.dev_data.state.lock().await;
      if !self.ensure_latest_build
        || (build_status.changed_files.is_empty() && !build_status.require_full_rebuild)
      {
        break build_status;
      }

      let mut changed_files = mem::take(&mut build_status.changed_files);
      if build_status.require_full_rebuild {
        changed_files.clear();
      }
      drop(build_status);
      let changed_files = changed_files.iter().map(|p| p.to_string_lossy().into()).collect();
      let scan_output = bundler.scan(changed_files).await?;
      let _bundle_output = bundler.bundle_write(scan_output).await?;
    };
    tracing::trace!(
      "`BuildStatus` finished building with changed files: {:#?}",
      self.changed_files
    );
    build_status.try_to_idle()?;
    Ok(())
  }
}
