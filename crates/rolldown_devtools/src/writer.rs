use std::{
  collections::hash_map::Entry,
  fs::{File, OpenOptions},
  io::{BufWriter, Write},
  path::PathBuf,
  sync::{
    Arc, LazyLock,
    mpsc::{Sender, channel},
  },
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use rustc_hash::{FxHashMap, FxHashSet};
use serde::ser::{SerializeMap, Serializer as _};

/// Commands sent to the background devtools log-writer thread.
pub enum LogCommand {
  /// Bind a session to the directory its relative log paths resolve against; the wasm binding has no process working directory, so relying on one writes outside the project.
  SetSessionCwd { session_id: String, cwd: PathBuf },
  /// Emit one event. Carries a fully resolved action payload plus the
  /// session/filename the producer has already decided on.
  Write { session_id: String, filename: Arc<str>, action_value: serde_json::Value },
  /// Flush and close every file associated with this session. When `ack` is
  /// `Some`, the writer signals it once all files for this session have been
  /// flushed to the OS, so callers can establish a happens-before relationship
  /// between "build finished" and "log file is readable".
  CloseSession { session_id: String, ack: Option<Sender<()>> },
}

static LOG_WRITER_TX: LazyLock<Sender<LogCommand>> = LazyLock::new(|| {
  let (tx, rx) = channel::<LogCommand>();
  thread::Builder::new()
    .name("rolldown-devtools-writer".into())
    .spawn(move || {
      let mut state = WriterState::default();
      while let Ok(cmd) = rx.recv() {
        state.handle(cmd);
      }
      // Channel closed (process exit): flush everything still held.
      state.flush_all();
    })
    .expect("failed to spawn rolldown-devtools-writer thread");
  tx
});

/// Fire-and-forget send to the writer thread. Producers never block on I/O.
pub fn send(cmd: LogCommand) {
  // If the writer thread has died, drop the command silently.
  let _ = LOG_WRITER_TX.send(cmd);
}

/// Request the writer thread to drain and flush every file for `session_id`,
/// returning a receiver that fires once the flush has completed. Consumers
/// use this to establish a happens-before relationship between `bundle.close()`
/// resolving and a reader opening the session's log files.
#[must_use = "the returned receiver must be awaited to actually wait for the flush"]
pub fn flush_session(session_id: String) -> std::sync::mpsc::Receiver<()> {
  let (tx, rx) = channel();
  send(LogCommand::CloseSession { session_id, ack: Some(tx) });
  rx
}

#[derive(Default)]
struct WriterState {
  files: FxHashMap<Arc<str>, BufWriter<File>>,
  files_by_session: FxHashMap<String, FxHashSet<Arc<str>>>,
  exist_hash_by_session: FxHashMap<String, FxHashSet<String>>,
  cwd_by_session: FxHashMap<String, PathBuf>,
  unopenable_files_by_session: FxHashMap<String, FxHashSet<Arc<str>>>,
}

impl WriterState {
  fn handle(&mut self, cmd: LogCommand) {
    match cmd {
      LogCommand::SetSessionCwd { session_id, cwd } => {
        self.cwd_by_session.insert(session_id, cwd);
      }
      LogCommand::Write { session_id, filename, action_value } => {
        let unopenable = self.unopenable_files_by_session.get(&session_id);
        if unopenable.is_some_and(|files| files.contains(&filename)) {
          return;
        }
        let file = match self.files.entry(Arc::clone(&filename)) {
          Entry::Occupied(entry) => entry.into_mut(),
          Entry::Vacant(entry) => {
            let path = self
              .cwd_by_session
              .get(&session_id)
              .map_or_else(|| PathBuf::from(filename.as_ref()), |cwd| cwd.join(filename.as_ref()));
            if let Some(parent) = path.parent() {
              let _ = std::fs::create_dir_all(parent);
            }
            match OpenOptions::new().create(true).append(true).open(&path) {
              Ok(file) => entry.insert(BufWriter::new(file)),
              // Losing devtools output must not abort the build; on wasm a panic here traps the whole module.
              Err(err) => {
                #[expect(clippy::print_stderr, reason = "no diagnostics channel on this thread")]
                {
                  eprintln!("devtools: failed to open log file {}: {err}", path.display());
                }
                // Warn once and stop retrying the open for every later event on this file.
                self.unopenable_files_by_session.entry(session_id).or_default().insert(filename);
                return;
              }
            }
          }
        };
        self.files_by_session.entry(session_id.clone()).or_default().insert(Arc::clone(&filename));
        let hashes = self.exist_hash_by_session.entry(session_id).or_default();
        let _ = write_event(file, &action_value, hashes);
      }
      LogCommand::CloseSession { session_id, ack } => {
        if let Some(files) = self.files_by_session.remove(&session_id) {
          for fname in files {
            if let Some(mut w) = self.files.remove(&fname) {
              let _ = w.flush();
            }
          }
        }
        // The session cwd outlives its files: `close()` queues this command before the `closeBundle` hooks run, so late events still have to resolve.
        self.exist_hash_by_session.remove(&session_id);
        // A late event on a still-broken file warns once more and re-adds its entry, which the next close drops again.
        self.unopenable_files_by_session.remove(&session_id);
        if let Some(ack) = ack {
          let _ = ack.send(());
        }
      }
    }
  }

  fn flush_all(&mut self) {
    for (_, mut w) in self.files.drain() {
      let _ = w.flush();
    }
  }
}

fn write_event(
  file: &mut BufWriter<File>,
  action_value: &serde_json::Value,
  exist_hashes: &mut FxHashSet<String>,
) -> Result<(), serde_json::Error> {
  let serde_json::Value::Object(action_meta) = action_value else {
    unreachable!("action_meta should always be an object")
  };

  // First pass: emit StringRef lines for any strings >5KB we haven't seen before.
  let mut wrote_ref = false;
  for value in action_meta.values() {
    if let serde_json::Value::String(s) = value {
      if s.len() > 5 * 1024 {
        let hash = blake3::hash(s.as_bytes()).to_hex().to_string();
        if exist_hashes.insert(hash.clone()) {
          let mut serializer = serde_json::Serializer::new(&mut *file);
          let mut map = serializer.serialize_map(None)?;
          map.serialize_entry("action", "StringRef")?;
          map.serialize_entry("id", &hash)?;
          map.serialize_entry("content", s)?;
          map.end()?;
          wrote_ref = true;
        }
      }
    }
  }
  if wrote_ref {
    writeln!(file).map_err(serde_json::Error::io)?;
  }

  // Second pass: emit the event line, with $ref:<hash> for strings >10KB.
  {
    let mut serializer = serde_json::Serializer::new(&mut *file);
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry("timestamp", &current_utc_timestamp_ms())?;
    for (key, value) in action_meta {
      match value {
        serde_json::Value::String(s) if s.len() > 10 * 1024 => {
          let hash = blake3::hash(s.as_bytes()).to_hex().to_string();
          map.serialize_entry(key, &format!("$ref:{hash}"))?;
        }
        _ => {
          map.serialize_entry(key, value)?;
        }
      }
    }
    map.end()?;
  }
  writeln!(file).map_err(serde_json::Error::io)?;

  Ok(())
}

fn current_utc_timestamp_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis()
}
