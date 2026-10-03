mod debounced;
mod event_map;
mod immediate;
mod noop;

use std::{fs, path::Path};

use notify::{Event, RecursiveMode, TargetMode, WatchMode, Watcher};
use notify_debouncer_full::{RecommendedCache, new_debouncer_opt};
use rolldown_error::{BuildResult, ResultExt};

use crate::{FsEventHandler, FsWatcherConfig};

pub trait WatcherBackend: Send {
  fn paths_mut(&mut self) -> Box<dyn PathsMut + '_>;
}

pub trait PathsMut {
  fn add(&mut self, path: &Path) -> BuildResult<()>;

  fn commit(self: Box<Self>) -> BuildResult<()>;
}

pub fn create_backend<F: FsEventHandler>(
  event_handler: F,
  config: &FsWatcherConfig,
) -> BuildResult<Box<dyn WatcherBackend>> {
  if !config.enabled {
    return Ok(Box::new(noop::NoopWatcher));
  }

  let notify_config = config.to_notify_config();
  Ok(match (config.use_polling, config.use_debounce) {
    (true, false) => Box::new(immediate::NotifyWatcher(
      ::notify::PollWatcher::new(NotifyEventHandlerAdapter(event_handler), notify_config)
        .map_err_to_unhandleable()?,
    )),
    (true, true) => Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::PollWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        NotifyEventHandlerAdapter(event_handler),
        RecommendedCache::new(),
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
    (false, false) => Box::new(immediate::NotifyWatcher(
      ::notify::RecommendedWatcher::new(NotifyEventHandlerAdapter(event_handler), notify_config)
        .map_err_to_unhandleable()?,
    )),
    (false, true) => Box::new(debounced::DebouncedNotifyWatcher(
      new_debouncer_opt::<_, ::notify::RecommendedWatcher, RecommendedCache>(
        config.debounce_delay_duration(),
        config.debounce_tick_rate(),
        NotifyEventHandlerAdapter(event_handler),
        RecommendedCache::new(),
        notify_config,
      )
      .map_err_to_unhandleable()?,
    )),
  })
}

struct NotifyPathsMutAdapter<'me>(Box<dyn ::notify::PathsMut + 'me>);

impl<'me> NotifyPathsMutAdapter<'me> {
  pub(super) fn new(paths_mut: Box<dyn ::notify::PathsMut + 'me>) -> Self {
    Self(paths_mut)
  }
}

impl PathsMut for NotifyPathsMutAdapter<'_> {
  fn add(&mut self, path: &Path) -> BuildResult<()> {
    // Windows makes a recursive file watch a parent subtree watch.
    // Root fix: rolldown notify fork.
    let recursive_mode = match fs::metadata(path) {
      Ok(metadata) if !metadata.is_dir() => RecursiveMode::NonRecursive,
      _ => RecursiveMode::Recursive,
    };
    self
      .0
      .add(path, WatchMode { recursive_mode, target_mode: TargetMode::TrackPath })
      .map_err_to_unhandleable()
      .map_err(Into::into)
  }

  fn commit(self: Box<Self>) -> BuildResult<()> {
    self.0.commit().map_err_to_unhandleable().map_err(Into::into)
  }
}

struct NotifyEventHandlerAdapter<T>(T);

impl<T: FsEventHandler> NotifyEventHandlerAdapter<T> {
  fn deliver(&mut self, notify_events: impl IntoIterator<Item = Event>) {
    let mut events = Vec::new();
    for notify_event in notify_events {
      event_map::map_notify_event(notify_event, &mut events);
    }
    if !events.is_empty() {
      self.0.handle_events(events);
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
  };

  use notify::{RecursiveMode, TargetMode, WatchMode};

  use super::{NotifyPathsMutAdapter, PathsMut};

  struct Recorder(Arc<Mutex<Vec<(PathBuf, WatchMode)>>>);

  impl ::notify::PathsMut for Recorder {
    fn add(&mut self, path: &Path, watch_mode: WatchMode) -> ::notify::Result<()> {
      self.0.lock().unwrap().push((path.to_path_buf(), watch_mode));
      Ok(())
    }

    fn remove(&mut self, _path: &Path) -> ::notify::Result<()> {
      Ok(())
    }

    fn commit(self: Box<Self>) -> ::notify::Result<()> {
      Ok(())
    }
  }

  #[test]
  fn watch_mode_by_target() {
    let dir = std::env::temp_dir()
      .join(format!("rolldown_fs_watcher_{}_watch_mode_by_target", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("a.js");
    fs::write(&file, "").unwrap();
    let missing = dir.join("missing.js");

    let recorded = Arc::new(Mutex::new(Vec::new()));
    let mut paths_mut = NotifyPathsMutAdapter::new(Box::new(Recorder(Arc::clone(&recorded))));
    paths_mut.add(&file).unwrap();
    paths_mut.add(&dir).unwrap();
    paths_mut.add(&missing).unwrap();
    Box::new(paths_mut).commit().unwrap();
    let _ = fs::remove_dir_all(&dir);

    let mode = |recursive_mode| WatchMode { recursive_mode, target_mode: TargetMode::TrackPath };
    assert_eq!(
      *recorded.lock().unwrap(),
      [
        (file, mode(RecursiveMode::NonRecursive)),
        (dir, mode(RecursiveMode::Recursive)),
        (missing, mode(RecursiveMode::Recursive)),
      ]
    );
  }
}
