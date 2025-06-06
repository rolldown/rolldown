// https://github.com/autozimu/LanguageClient-neovim/blob/cf6dd11baf62fb6ce18308e96c0ab43428b7c686/src/watcher.rs

use anyhow::{Result, anyhow};
use notify::{DebouncedEvent, RecursiveMode, Watcher};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// FSWatch has an outgoing channel on which it pushes events for any of the files or directories it
// is currently watching. Both directory and file watches are implemented using directory watches,
// in order to be able to catch write-via-rename tricks like Vim does on save.
//
// Thus, FSWatch has a number of directory watches in place. These are all non-recursive watches;
// recursive watches are connected directly to the FSWatch outgoing channel.
// There is one thread, with entry point fswatch_service(), that collects all notify events and
// filters out the ones that we're not interested in. Its incoming channel is attached to the
// notify::RecommendedWatcher, and the outgoing channel is the outgoing channel of FSWatch.

struct DirWatch {
  /// whether to also watch the directory itself
  full_directory: bool,
  files: HashSet<String>,
}

fn interested(dirs: &Arc<Mutex<HashMap<PathBuf, DirWatch>>>, path: &Path) -> Result<bool> {
  let dirs = dirs.lock().map_err(|err| anyhow!("Failed to lock watcher: {:?}", err))?;

  if let Some(dw) = dirs.get(path) {
    if dw.full_directory {
      return Ok(true);
    }
  }

  if let Some(parent) = path.parent() {
    if let Some(dw) = dirs.get(parent) {
      if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        return Ok(dw.files.contains(name));
      }
    }
  }

  Ok(false)
}

fn fswatch_service(
  rx: mpsc::Receiver<DebouncedEvent>,
  tx: mpsc::Sender<DebouncedEvent>,
  dirs: Arc<Mutex<HashMap<PathBuf, DirWatch>>>,
) -> Result<()> {
  let mut notice_remove = HashSet::new();

  // Policy:
  // - We never push through NoticeRemove events, since these may be spurious in the case of
  //   write-via-rename tricks, and if the remove was real we'll get a real Remove event later
  //   anyway.
  // - When getting a different event, we always remove the path from notice_remove if
  //   applicable. (Even if we're not interested now, we may have been in the past, and we
  //   want to avoid memory leaks).
  // - Then, if we're interested in a path, we handle and push through the event, possibly
  //   modified by the knowledge that we caught a NoticeRemove earlier.
  for event in rx {
    match &event {
      DebouncedEvent::NoticeWrite(path) => {
        notice_remove.remove(path);
        if !interested(&dirs, path)? {
          continue;
        }
        tx.send(event)?;
      }

      DebouncedEvent::NoticeRemove(path) => {
        if !interested(&dirs, path)? {
          continue;
        }
        notice_remove.insert(path.to_owned());
      }

      DebouncedEvent::Create(path) => {
        let interest = interested(&dirs, path)?;
        // Detect and handle Vim's write-via-rename trick
        if notice_remove.remove(path) {
          if interest {
            tx.send(DebouncedEvent::Write(path.to_owned()))?;
          }
        } else if interest {
          tx.send(event)?;
        }
      }

      DebouncedEvent::Write(path) => {
        notice_remove.remove(path);
        if !interested(&dirs, path)? {
          continue;
        }
        tx.send(event)?;
      }

      DebouncedEvent::Chmod(path) => {
        notice_remove.remove(path);
        if !interested(&dirs, path)? {
          continue;
        }
        tx.send(event)?;
      }

      DebouncedEvent::Remove(path) => {
        notice_remove.remove(path);
        if !interested(&dirs, path)? {
          continue;
        }
        tx.send(event)?;
      }

      DebouncedEvent::Rename(from, to) => {
        notice_remove.remove(from);
        let interest_from = interested(&dirs, from)?;
        let interest_to = interested(&dirs, to)?;
        // TODO: is this the right behaviour?
        if notice_remove.remove(to) && interest_to {
          tx.send(DebouncedEvent::Remove(to.clone()))?;
        }
        if interest_from {
          if interest_to {
            tx.send(event)?;
          } else {
            tx.send(DebouncedEvent::Remove(from.to_owned()))?;
          }
        } else if interest_to {
          tx.send(DebouncedEvent::Create(to.to_owned()))?;
        }
      }

      DebouncedEvent::Rescan => {
        tx.send(event)?;
      }

      DebouncedEvent::Error(_, optpath) => {
        // TODO: is this the right behaviour?
        if let Some(path) = optpath {
          let interest = interested(&dirs, path)?;
          if notice_remove.remove(path) && interest {
            tx.send(DebouncedEvent::Remove(path.to_owned()))?;
          }
          if !interest {
            continue;
          }
        }
        tx.send(event)?;
      }
    }
  }

  Ok(())
}

enum UnwatchInfo {
  /// Recursive mode (directory is just the watched path itself)
  Directory(RecursiveMode),
  /// Directory and file
  File(PathBuf, String),
}

pub struct FSWatch {
  dirs: Arc<Mutex<HashMap<PathBuf, DirWatch>>>,
  watcher: notify::RecommendedWatcher,

  // Used for recursive directory watches; is connected directly to the event sink.
  recursive_watcher: Option<notify::RecommendedWatcher>,
  event_sink: mpsc::Sender<DebouncedEvent>,
  delay: Duration,

  unwatch_info: HashMap<PathBuf, UnwatchInfo>,
}

impl FSWatch {
  pub fn new(event_sink: mpsc::Sender<DebouncedEvent>, delay: Duration) -> Result<Self> {
    let (funnel_tx, funnel_rx) = mpsc::channel();
    let dirs = Arc::new(Mutex::new(HashMap::new()));
    let dirs_clone = dirs.clone();
    let event_sink_clone = event_sink.clone();
    thread::spawn(move || fswatch_service(funnel_rx, event_sink_clone, dirs_clone));
    let watcher = notify::watcher(funnel_tx, delay)?;
    Ok(Self {
      dirs,
      watcher,
      recursive_watcher: None,
      event_sink,
      delay,
      unwatch_info: HashMap::new(),
    })
  }

  /// 'path' must be a file, not a directory.
  pub fn watch_file<P: AsRef<Path> + std::fmt::Debug>(&mut self, path: P) -> Result<()> {
    let err_msg = || Err(anyhow!("FSWatch::watch_file on an invalid path"));

    let dirname = if let Some(x) = path.as_ref().parent() {
      x
    } else {
      return err_msg();
    };
    let name = if let Some(x) = path.as_ref().file_name().and_then(|n| n.to_str()) {
      x
    } else {
      return err_msg();
    };

    self
      .unwatch_info
      .insert(path.as_ref().to_owned(), UnwatchInfo::File(dirname.to_owned(), name.to_owned()));

    let mut dirs = self.dirs.lock().map_err(|err| anyhow!("Failed to lock watcher: {:?}", err))?;
    match dirs.get_mut(dirname) {
      Some(dw) => {
        dw.files.insert(name.to_string());
        Ok(())
      }
      None => {
        // watch first; if this throws an error, don't insert into the 'dirs' structure
        self.watcher.watch(dirname, RecursiveMode::NonRecursive)?;

        let mut files = HashSet::new();
        files.insert(name.to_string());
        dirs.insert(dirname.to_owned(), DirWatch { full_directory: false, files });
        Ok(())
      }
    }
  }

  /// 'path' must be a directory, not a file.
  pub fn watch_dir<P: AsRef<Path> + std::fmt::Debug>(
    &mut self,
    path: P,
    recurse: RecursiveMode,
  ) -> Result<()> {
    self.unwatch_info.insert(path.as_ref().to_owned(), UnwatchInfo::Directory(recurse));

    match recurse {
      RecursiveMode::Recursive => match &mut self.recursive_watcher {
        Some(w) => {
          w.watch(path, RecursiveMode::Recursive)?;
          Ok(())
        }
        None => {
          let mut w = notify::watcher(self.event_sink.clone(), self.delay)?;
          w.watch(path, RecursiveMode::Recursive)?;
          self.recursive_watcher = Some(w);
          Ok(())
        }
      },

      RecursiveMode::NonRecursive => {
        let mut dirs =
          self.dirs.lock().map_err(|err| anyhow!("Failed to lock watcher: {:?}", err))?;
        match dirs.get_mut(path.as_ref()) {
          Some(dw) => {
            dw.full_directory = true;
            Ok(())
          }
          None => {
            // watch first; if this throws an error, don't insert into the 'dirs' structure
            self.watcher.watch(&path, RecursiveMode::NonRecursive)?;

            dirs.insert(
              path.as_ref().to_owned(),
              DirWatch { full_directory: true, files: HashSet::new() },
            );
            Ok(())
          }
        }
      }
    }
  }

  pub fn unwatch<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
    let info = match self.unwatch_info.get(path.as_ref()) {
      Some(info) => info,
      None => return Err(anyhow!("FSWatch::unwatch on a non-watched path")),
    };

    let (key, filename): (&Path, Option<&str>) = match info {
      UnwatchInfo::File(key, filename) => (&key, Some(filename)),
      UnwatchInfo::Directory(RecursiveMode::NonRecursive) => (path.as_ref(), None),
      UnwatchInfo::Directory(RecursiveMode::Recursive) => {
        self
          .recursive_watcher
          .as_mut()
          .ok_or_else(|| anyhow!("Unexpected watcher state: not initialized"))?
          .unwatch(path)?;
        return Ok(());
      }
    };

    let mut dirs = self.dirs.lock().map_err(|err| anyhow!("Failed to lock watcher: {:?}", err))?;
    let mut dw =
      dirs.get_mut(key).ok_or_else(|| anyhow!("Unexpected watcher state: file not watched"))?;
    match filename {
      Some(filename) => {
        dw.files.remove(filename);
      }
      None => {
        dw.full_directory = false;
      }
    }

    Ok(())
  }
}
