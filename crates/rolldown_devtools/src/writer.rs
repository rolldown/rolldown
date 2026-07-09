use std::{
  any::Any,
  error::Error,
  fmt,
  fs::{File, OpenOptions},
  io::{self, BufWriter, Write},
  panic::{AssertUnwindSafe, catch_unwind},
  path::{Component, Path, PathBuf},
  sync::{
    Arc, LazyLock, Mutex,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, Sender, channel},
  },
  time::{SystemTime, UNIX_EPOCH},
};

use rustc_hash::{FxHashMap, FxHashSet};
use serde::ser::{SerializeMap, Serializer as _};

static SESSION_OWNER_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DevtoolsSessionKey {
  session: DevtoolsLogicalSessionKey,
  owner_id: u64,
}

impl DevtoolsSessionKey {
  pub fn new(session_id: Arc<str>, cwd: &Path) -> Self {
    Self {
      session: DevtoolsLogicalSessionKey::from_cwd(session_id, cwd),
      owner_id: SESSION_OWNER_ID.fetch_add(1, Ordering::Relaxed),
    }
  }

  pub fn output_root(&self) -> &str {
    self.session.output_root()
  }

  pub fn session_id(&self) -> &str {
    self.session.session_id()
  }

  fn logical_session(&self) -> &DevtoolsLogicalSessionKey {
    &self.session
  }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DevtoolsLogicalSessionKey {
  output_root: Arc<str>,
  session_id: Arc<str>,
}

impl DevtoolsLogicalSessionKey {
  pub(crate) fn from_cwd(session_id: Arc<str>, cwd: &Path) -> Self {
    Self { output_root: canonical_output_root(cwd), session_id }
  }

  pub(crate) fn from_output_root(session_id: Arc<str>, output_root: Arc<str>) -> Self {
    Self { output_root, session_id }
  }

  pub(crate) fn output_root(&self) -> &str {
    &self.output_root
  }

  pub(crate) fn session_id(&self) -> &str {
    &self.session_id
  }

  pub(crate) fn log_filename(&self, is_session_meta: bool) -> Arc<str> {
    let filename = if is_session_meta { "meta.json" } else { "logs.json" };
    Path::new(self.output_root())
      .join(safe_session_path_component(self.session_id()))
      .join(filename)
      .to_string_lossy()
      .into_owned()
      .into()
  }
}

fn canonical_output_root(cwd: &Path) -> Arc<str> {
  let cwd = if cwd.as_os_str().is_empty() {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from(Component::CurDir.as_os_str()))
  } else {
    cwd.to_path_buf()
  };
  let absolute_cwd = if cwd.is_absolute() {
    cwd
  } else {
    std::env::current_dir().map_or(cwd.clone(), |current_dir| current_dir.join(cwd))
  };
  let canonical_cwd =
    std::fs::canonicalize(&absolute_cwd).unwrap_or_else(|_| normalize_path(&absolute_cwd));
  canonical_cwd.join("node_modules").join(".rolldown").to_string_lossy().into_owned().into()
}

fn normalize_path(path: &Path) -> PathBuf {
  let mut normalized = PathBuf::new();
  for component in path.components() {
    match component {
      Component::CurDir => {}
      Component::ParentDir => {
        if matches!(normalized.components().next_back(), Some(Component::Normal(_))) {
          normalized.pop();
        } else if !normalized.has_root() {
          normalized.push(component.as_os_str());
        }
      }
      Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
        normalized.push(component.as_os_str());
      }
    }
  }
  normalized
}

fn safe_session_path_component(session_id: &str) -> String {
  const MAX_PORTABLE_COMPONENT_BYTES: usize = 200;
  const MAX_HEX_ENCODED_BYTES: usize = 95;

  if !session_id.is_empty()
    && session_id.len() <= MAX_PORTABLE_COMPONENT_BYTES
    && session_id
      .bytes()
      .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_'))
    && !is_windows_reserved_name(session_id)
  {
    return session_id.to_string();
  }

  if session_id.len() <= MAX_HEX_ENCODED_BYTES {
    let mut encoded = String::with_capacity(1 + session_id.len() * 2);
    encoded.push('~');
    for byte in session_id.bytes() {
      use std::fmt::Write as _;
      write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    return encoded;
  }

  format!("~h{}", blake3::hash(session_id.as_bytes()).to_hex())
}

fn is_windows_reserved_name(value: &str) -> bool {
  matches!(
    value.to_ascii_uppercase().as_str(),
    "CON"
      | "PRN"
      | "AUX"
      | "NUL"
      | "COM1"
      | "COM2"
      | "COM3"
      | "COM4"
      | "COM5"
      | "COM6"
      | "COM7"
      | "COM8"
      | "COM9"
      | "LPT1"
      | "LPT2"
      | "LPT3"
      | "LPT4"
      | "LPT5"
      | "LPT6"
      | "LPT7"
      | "LPT8"
      | "LPT9"
  )
}

/// Commands sent to the devtools log-writer backend.
pub enum LogCommand {
  RegisterSessionOwner { session: DevtoolsSessionKey },
  Write { session: DevtoolsLogicalSessionKey, filename: Arc<str>, action_value: serde_json::Value },
  CloseSession { session: DevtoolsSessionKey, ack: Option<Sender<DevtoolsWriterResult>> },
}

enum WriterBackend {
  #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
  Ready(Sender<LogCommand>),
  Synchronous(Mutex<WriterState>),
  #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
  Failed(DevtoolsWriterFailure),
}

impl WriterBackend {
  fn start() -> Self {
    #[cfg(all(target_family = "wasm", not(rolldown_wasi_threads)))]
    {
      Self::Synchronous(Mutex::new(WriterState::default()))
    }
    #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
    {
      let (tx, rx) = channel::<LogCommand>();
      match std::thread::Builder::new().name("rolldown-devtools-writer".into()).spawn(move || {
        let mut state = WriterState::default();
        while let Ok(cmd) = rx.recv() {
          state.handle(cmd);
        }
        state.flush_all();
      }) {
        Ok(_) => Self::Ready(tx),
        Err(error) => Self::Failed(DevtoolsWriterFailure::new(
          DevtoolsWriterOperation::StartWriter,
          "rolldown-devtools-writer".into(),
          error,
        )),
      }
    }
  }
}

static LOG_WRITER_BACKEND: LazyLock<WriterBackend> = LazyLock::new(WriterBackend::start);

pub fn register_session_owner(session: DevtoolsSessionKey) {
  send_best_effort(LogCommand::RegisterSessionOwner { session });
}

/// Fire-and-forget submission for ordinary writes and cleanup fallbacks.
pub fn send(cmd: LogCommand) {
  send_best_effort(cmd);
}

pub fn send_best_effort(cmd: LogCommand) {
  send_best_effort_to(&LOG_WRITER_BACKEND, cmd);
}

fn send_best_effort_to<F>(backend: &LazyLock<WriterBackend, F>, cmd: LogCommand)
where
  F: FnOnce() -> WriterBackend,
{
  if let Err(payload) = catch_unwind(AssertUnwindSafe(|| match &**backend {
    #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
    WriterBackend::Ready(sender) => {
      let _ = sender.send(cmd);
    }
    WriterBackend::Synchronous(state) => {
      if let Ok(mut state) = state.lock() {
        state.handle(cmd);
      }
    }
    #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
    WriterBackend::Failed(_) => {}
  })) {
    discard_panic_payload(payload);
  }
}

/// Request the writer backend to drain this owner's logical session, returning
/// a receiver that reports retained write and flush failures after cleanup.
#[must_use = "the returned receiver must be awaited to actually wait for the flush"]
pub fn flush_session(session: DevtoolsSessionKey) -> Receiver<DevtoolsWriterResult> {
  flush_session_with_backend(session, &LOG_WRITER_BACKEND)
}

fn flush_session_with_backend<F>(
  session: DevtoolsSessionKey,
  backend: &LazyLock<WriterBackend, F>,
) -> Receiver<DevtoolsWriterResult>
where
  F: FnOnce() -> WriterBackend,
{
  let session_id = session.session_id().to_string();
  let (ack, rx) = channel();
  let fallback_ack = ack.clone();
  let result = catch_unwind(AssertUnwindSafe(|| match &**backend {
    #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
    WriterBackend::Ready(sender) => {
      sender.send(LogCommand::CloseSession { session, ack: Some(ack) }).map_err(|_| {
        DevtoolsWriterFailure::message(
          DevtoolsWriterOperation::SubmitCommand,
          "rolldown-devtools-writer".into(),
          "writer thread disconnected before accepting the close command",
        )
      })
    }
    WriterBackend::Synchronous(state) => {
      let mut state = state.lock().map_err(|_| {
        DevtoolsWriterFailure::message(
          DevtoolsWriterOperation::AccessWriter,
          "rolldown-devtools-writer".into(),
          "synchronous writer state was poisoned",
        )
      })?;
      state.handle(LogCommand::CloseSession { session, ack: Some(ack) });
      Ok(())
    }
    #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
    WriterBackend::Failed(failure) => Err(failure.clone()),
  }));

  match result {
    Ok(Ok(())) => {}
    Ok(Err(failure)) => {
      let _ = fallback_ack.send(Err(DevtoolsWriterError::from_failure(session_id, failure)));
    }
    Err(payload) => {
      let failure = panic_failure(
        DevtoolsWriterOperation::AccessWriter,
        "rolldown-devtools-writer".into(),
        payload,
      );
      let _ = fallback_ack.send(Err(DevtoolsWriterError::from_failure(session_id, failure)));
    }
  }
  rx
}

fn panic_payload_message(payload: &(dyn Any + Send)) -> String {
  if let Some(message) = payload.downcast_ref::<String>() {
    message.clone()
  } else if let Some(message) = payload.downcast_ref::<&str>() {
    (*message).to_string()
  } else {
    "non-string panic payload".to_string()
  }
}

fn discard_panic_payload(payload: Box<dyn Any + Send>) {
  if let Err(nested_payload) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
    std::mem::forget(nested_payload);
  }
}

fn panic_failure(
  operation: DevtoolsWriterOperation,
  path: Arc<str>,
  payload: Box<dyn Any + Send>,
) -> DevtoolsWriterFailure {
  let message = panic_payload_message(&*payload);
  discard_panic_payload(payload);
  DevtoolsWriterFailure::message(operation, path, format!("writer backend panicked: {message}"))
}

pub type DevtoolsWriterResult = Result<(), DevtoolsWriterError>;

#[derive(Debug)]
pub struct DevtoolsWriterError {
  session_id: String,
  failures: Box<[DevtoolsWriterFailure]>,
}

impl DevtoolsWriterError {
  fn from_failure(session_id: String, failure: DevtoolsWriterFailure) -> Self {
    Self { session_id, failures: Box::new([failure]) }
  }

  pub fn session_id(&self) -> &str {
    &self.session_id
  }

  pub fn failures(&self) -> &[DevtoolsWriterFailure] {
    &self.failures
  }

  pub fn into_failures(self) -> Box<[DevtoolsWriterFailure]> {
    self.failures
  }
}

impl fmt::Display for DevtoolsWriterError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "devtools writer failed for session `{}`:\n- {}",
      self.session_id,
      self.failures.iter().map(std::string::ToString::to_string).collect::<Vec<_>>().join("\n- ")
    )
  }
}

impl Error for DevtoolsWriterError {}

#[derive(Clone, Debug)]
pub struct DevtoolsWriterFailure {
  operation: DevtoolsWriterOperation,
  path: Arc<str>,
  source: Arc<dyn Error + Send + Sync>,
}

impl DevtoolsWriterFailure {
  fn new<E>(operation: DevtoolsWriterOperation, path: Arc<str>, source: E) -> Self
  where
    E: Error + Send + Sync + 'static,
  {
    Self { operation, path, source: Arc::new(source) }
  }

  fn message(
    operation: DevtoolsWriterOperation,
    path: Arc<str>,
    message: impl Into<String>,
  ) -> Self {
    Self::new(operation, path, MessageError(message.into()))
  }

  pub fn operation(&self) -> DevtoolsWriterOperation {
    self.operation
  }

  pub fn path(&self) -> &str {
    &self.path
  }
}

impl fmt::Display for DevtoolsWriterFailure {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "failed to {} `{}`: {}", self.operation, self.path, self.source)
  }
}

impl Error for DevtoolsWriterFailure {
  fn source(&self) -> Option<&(dyn Error + 'static)> {
    Some(self.source.as_ref())
  }
}

#[derive(Debug)]
struct MessageError(String);

impl fmt::Display for MessageError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl Error for MessageError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevtoolsWriterOperation {
  StartWriter,
  AccessWriter,
  SubmitCommand,
  ProcessCommand,
  CreateDirectory,
  OpenFile,
  WriteEvent,
  FlushFile,
}

impl fmt::Display for DevtoolsWriterOperation {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(match self {
      Self::StartWriter => "start writer thread",
      Self::AccessWriter => "access writer backend",
      Self::SubmitCommand => "submit command to",
      Self::ProcessCommand => "process writer command for",
      Self::CreateDirectory => "create directory",
      Self::OpenFile => "open file",
      Self::WriteEvent => "write event to",
      Self::FlushFile => "flush file",
    })
  }
}

struct WriterFile<W: Write> {
  writer: BufWriter<W>,
  hashes: FxHashSet<String>,
}

// See internal-docs/devtools/implementation.md.
struct WriterState<W: Write = File> {
  files: FxHashMap<Arc<str>, WriterFile<W>>,
  files_by_session: FxHashMap<DevtoolsLogicalSessionKey, FxHashSet<Arc<str>>>,
  dir_ensured: FxHashSet<DevtoolsLogicalSessionKey>,
  owners_by_session: FxHashMap<DevtoolsLogicalSessionKey, FxHashSet<DevtoolsSessionKey>>,
  failures_by_session: FxHashMap<DevtoolsLogicalSessionKey, Vec<DevtoolsWriterFailure>>,
  failures_by_owner: FxHashMap<DevtoolsSessionKey, Vec<DevtoolsWriterFailure>>,
}

impl<W: Write> Default for WriterState<W> {
  fn default() -> Self {
    Self {
      files: FxHashMap::default(),
      files_by_session: FxHashMap::default(),
      dir_ensured: FxHashSet::default(),
      owners_by_session: FxHashMap::default(),
      failures_by_session: FxHashMap::default(),
      failures_by_owner: FxHashMap::default(),
    }
  }
}

impl WriterState<File> {
  fn handle(&mut self, cmd: LogCommand) {
    match cmd {
      LogCommand::RegisterSessionOwner { session } => self.register_owner(session),
      LogCommand::Write { session, filename, action_value } => {
        self.write_contained(
          &session,
          filename,
          &action_value,
          |directory| std::fs::create_dir_all(directory),
          |filename| {
            let file = OpenOptions::new().create(true).append(true).open(filename)?;
            Ok(BufWriter::new(file))
          },
        );
      }
      LogCommand::CloseSession { session, ack } => {
        let result = self.close_session(&session);
        if let Some(ack) = ack {
          let _ = ack.send(result);
        }
      }
    }
  }
}

impl<W: Write> WriterState<W> {
  fn register_owner(&mut self, owner: DevtoolsSessionKey) {
    let session = owner.logical_session().clone();
    let owners = self.owners_by_session.entry(session.clone()).or_default();
    if !owners.insert(owner.clone()) {
      return;
    }
    let retained = self.failures_by_session.get(&session).cloned().unwrap_or_default();
    self.failures_by_owner.insert(owner, retained);
  }

  fn write_contained<EnsureDirectory, OpenFile>(
    &mut self,
    session: &DevtoolsLogicalSessionKey,
    filename: Arc<str>,
    action_value: &serde_json::Value,
    ensure_directory: EnsureDirectory,
    open_file: OpenFile,
  ) where
    EnsureDirectory: FnOnce(&Path) -> io::Result<()>,
    OpenFile: FnOnce(&str) -> io::Result<BufWriter<W>>,
  {
    let failure_path = Arc::clone(&filename);
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
      self.write(session, filename, action_value, ensure_directory, open_file);
    })) {
      self.record_failure(
        session,
        &panic_failure(DevtoolsWriterOperation::ProcessCommand, failure_path, payload),
      );
    }
  }

  fn write<EnsureDirectory, OpenFile>(
    &mut self,
    session: &DevtoolsLogicalSessionKey,
    filename: Arc<str>,
    action_value: &serde_json::Value,
    ensure_directory: EnsureDirectory,
    open_file: OpenFile,
  ) where
    EnsureDirectory: FnOnce(&Path) -> io::Result<()>,
    OpenFile: FnOnce(&str) -> io::Result<BufWriter<W>>,
  {
    if self.owners_by_session.get(session).is_none_or(FxHashSet::is_empty) {
      return;
    }

    if !self.dir_ensured.contains(session)
      && let Some(parent) = Path::new(filename.as_ref()).parent()
    {
      match ensure_directory(parent) {
        Ok(()) => {
          self.dir_ensured.insert(session.clone());
        }
        Err(error) => {
          self.record_failure(
            session,
            &DevtoolsWriterFailure::new(
              DevtoolsWriterOperation::CreateDirectory,
              parent.to_string_lossy().into_owned().into(),
              error,
            ),
          );
        }
      }
    }

    if !self.files.contains_key(&filename) {
      match open_file(filename.as_ref()) {
        Ok(writer) => {
          self
            .files
            .insert(Arc::clone(&filename), WriterFile { writer, hashes: FxHashSet::default() });
        }
        Err(error) => {
          self.record_failure(
            session,
            &DevtoolsWriterFailure::new(DevtoolsWriterOperation::OpenFile, filename, error),
          );
          return;
        }
      }
    }

    self.files_by_session.entry(session.clone()).or_default().insert(Arc::clone(&filename));

    let write_result = {
      let file = self.files.get_mut(&filename).expect("file was opened above");
      write_event(&mut file.writer, action_value, &mut file.hashes)
    };
    if let Err(error) = write_result {
      self.record_failure(
        session,
        &DevtoolsWriterFailure::new(DevtoolsWriterOperation::WriteEvent, filename, error),
      );
    }
  }

  fn close_session(&mut self, owner: &DevtoolsSessionKey) -> DevtoolsWriterResult {
    let session = owner.logical_session();
    let Some((is_active, is_final_owner)) =
      self.owners_by_session.get(session).map(|owners| (owners.contains(owner), owners.len() == 1))
    else {
      return Ok(());
    };
    if !is_active {
      return Ok(());
    }

    self.flush_session_files(session, is_final_owner);
    let failures = self.failures_by_owner.remove(owner).unwrap_or_default();

    if let Some(owners) = self.owners_by_session.get_mut(session) {
      owners.remove(owner);
      if owners.is_empty() {
        self.owners_by_session.remove(session);
      }
    }

    if is_final_owner {
      self.files_by_session.remove(session);
      self.dir_ensured.remove(session);
      self.failures_by_session.remove(session);
    }

    if failures.is_empty() {
      Ok(())
    } else {
      Err(DevtoolsWriterError {
        session_id: session.session_id().to_string(),
        failures: failures.into_boxed_slice(),
      })
    }
  }

  fn flush_session_files(&mut self, session: &DevtoolsLogicalSessionKey, remove: bool) {
    let mut filenames = self
      .files_by_session
      .get(session)
      .map_or_else(Vec::new, |files| files.iter().cloned().collect());
    filenames.sort_unstable_by(|a, b| a.as_ref().cmp(b.as_ref()));

    let mut failures = Vec::new();
    for filename in filenames {
      let failure_path = Arc::clone(&filename);
      let flush_result = catch_unwind(AssertUnwindSafe(|| {
        if remove {
          self.files.remove(&filename).map(|mut file| file.writer.flush())
        } else {
          self.files.get_mut(&filename).map(|file| file.writer.flush())
        }
      }));
      match flush_result {
        Ok(Some(Err(error))) => {
          failures.push(DevtoolsWriterFailure::new(
            DevtoolsWriterOperation::FlushFile,
            failure_path,
            error,
          ));
        }
        Err(payload) => {
          failures.push(panic_failure(DevtoolsWriterOperation::FlushFile, failure_path, payload));
        }
        Ok(None | Some(Ok(()))) => {}
      }
    }
    for failure in failures {
      self.record_failure(session, &failure);
    }
  }

  fn record_failure(
    &mut self,
    session: &DevtoolsLogicalSessionKey,
    failure: &DevtoolsWriterFailure,
  ) {
    let retained = self.failures_by_session.entry(session.clone()).or_default();
    if retained
      .iter()
      .any(|existing| existing.operation == failure.operation && existing.path == failure.path)
    {
      return;
    }
    retained.push(failure.clone());
    if let Some(owners) = self.owners_by_session.get(session) {
      for owner in owners {
        self.failures_by_owner.entry(owner.clone()).or_default().push(failure.clone());
      }
    }
  }

  #[cfg(not(all(target_family = "wasm", not(rolldown_wasi_threads))))]
  fn flush_all(&mut self) {
    let mut owners =
      self.owners_by_session.values().flat_map(FxHashSet::iter).cloned().collect::<Vec<_>>();
    owners.sort_unstable_by_key(|owner| owner.owner_id);
    for owner in owners {
      let _ = self.close_session(&owner);
    }

    for (_, mut file) in self.files.drain() {
      let _ = file.writer.flush();
    }
    self.files_by_session.clear();
    self.dir_ensured.clear();
    self.owners_by_session.clear();
    self.failures_by_session.clear();
    self.failures_by_owner.clear();
  }
}

fn write_event<W: Write>(
  file: &mut W,
  action_value: &serde_json::Value,
  existing_hashes: &mut FxHashSet<String>,
) -> Result<(), serde_json::Error> {
  let serde_json::Value::Object(action_meta) = action_value else {
    unreachable!("action_meta should always be an object")
  };

  for (key, value) in action_meta {
    if is_structural_identity_field(key) {
      continue;
    }
    if let serde_json::Value::String(content) = value
      && content.len() > 5 * 1024
    {
      let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
      #[expect(
        clippy::set_contains_or_insert,
        reason = "the hash must only be inserted after its complete StringRef line is written"
      )]
      if !existing_hashes.contains(&hash) {
        write_json_line(file, |serializer| {
          let mut map = serializer.serialize_map(None)?;
          map.serialize_entry("action", "StringRef")?;
          map.serialize_entry("id", &hash)?;
          map.serialize_entry("content", content)?;
          map.end()
        })?;
        existing_hashes.insert(hash);
      }
    }
  }

  write_json_line(file, |serializer| {
    let mut map = serializer.serialize_map(None)?;
    map.serialize_entry("timestamp", &current_utc_timestamp_ms())?;
    for (key, value) in action_meta {
      match value {
        serde_json::Value::String(content)
          if !is_structural_identity_field(key) && content.len() > 10 * 1024 =>
        {
          let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
          map.serialize_entry(key, &format!("$ref:{hash}"))?;
        }
        _ => {
          map.serialize_entry(key, value)?;
        }
      }
    }
    map.end()
  })
}

fn is_structural_identity_field(key: &str) -> bool {
  matches!(key, "action" | "build_id" | "session_id")
}

fn write_json_line(
  file: &mut impl Write,
  serialize: impl FnOnce(&mut serde_json::Serializer<&mut Vec<u8>>) -> Result<(), serde_json::Error>,
) -> Result<(), serde_json::Error> {
  let mut line = Vec::new();
  serialize(&mut serde_json::Serializer::new(&mut line))?;
  line.push(b'\n');
  file.write_all(&line).map_err(serde_json::Error::io)
}

fn current_utc_timestamp_ms() -> u128 {
  SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

#[cfg(test)]
mod tests {
  use std::{
    cell::Cell,
    fs,
    panic::panic_any,
    sync::atomic::{AtomicU64, Ordering},
  };

  use super::*;

  static TEST_PATH_ID: AtomicU64 = AtomicU64::new(0);

  fn temp_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
      "rolldown-devtools-{name}-{}-{}",
      std::process::id(),
      TEST_PATH_ID.fetch_add(1, Ordering::Relaxed)
    ))
  }

  fn session_key(cwd: impl AsRef<Path>, session_id: &str) -> DevtoolsSessionKey {
    DevtoolsSessionKey::new(session_id.into(), cwd.as_ref())
  }

  #[derive(Debug)]
  struct TestWriter {
    fail_write: bool,
    fail_flush: bool,
  }

  impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      if self.fail_write { Err(io::Error::other("injected write failure")) } else { Ok(buf.len()) }
    }

    fn flush(&mut self) -> io::Result<()> {
      if self.fail_flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
  }

  #[derive(Debug)]
  struct PanickingWriter;

  impl Write for PanickingWriter {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
      panic!("injected writer panic");
    }

    fn flush(&mut self) -> io::Result<()> {
      Ok(())
    }
  }

  #[derive(Debug)]
  struct PanickingFlushWriter;

  impl Write for PanickingFlushWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
      Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
      panic!("injected flush panic");
    }
  }

  #[derive(Debug)]
  struct PanickingPayloadDrop;

  impl Drop for PanickingPayloadDrop {
    fn drop(&mut self) {
      panic!("injected panic payload drop");
    }
  }

  #[test]
  fn ordinary_writer_access_contains_lazy_backend_panics() {
    let backend = LazyLock::<WriterBackend>::new(|| panic_any(PanickingPayloadDrop));
    let cwd = temp_path("backend-panic");
    let owner = session_key(&cwd, "session");

    send_best_effort_to(&backend, LogCommand::RegisterSessionOwner { session: owner.clone() });
    send_best_effort_to(
      &backend,
      LogCommand::Write {
        session: owner.logical_session().clone(),
        filename: owner.logical_session().log_filename(false),
        action_value: serde_json::json!({ "action": "BuildStart" }),
      },
    );

    let error = flush_session_with_backend(owner, &backend)
      .recv()
      .expect("receive immediate backend failure")
      .expect_err("poisoned backend should fail");
    assert_eq!(error.failures()[0].operation(), DevtoolsWriterOperation::AccessWriter);
  }

  #[test]
  fn ordinary_write_processing_panic_is_retained_and_cleaned() {
    let cwd = temp_path("write-panic");
    fs::create_dir_all(&cwd).expect("create cwd");
    let owner = session_key(&cwd, "session");
    let session = owner.logical_session().clone();
    let mut state = WriterState::<PanickingWriter>::default();
    state.register_owner(owner.clone());
    state.write_contained(
      &session,
      session.log_filename(false),
      &serde_json::json!({ "action": "BuildStart" }),
      |_| Ok(()),
      |_| Ok(BufWriter::with_capacity(0, PanickingWriter)),
    );

    let error = state.close_session(&owner).expect_err("write panic should be retained");
    assert_eq!(error.failures()[0].operation(), DevtoolsWriterOperation::ProcessCommand);
    assert_session_clean(&state, &session);
    assert!(state.files.is_empty());
    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn session_flush_panic_is_retained_acknowledged_and_cleaned() {
    let cwd = temp_path("flush-panic");
    fs::create_dir_all(&cwd).expect("create cwd");
    let owner = session_key(&cwd, "session");
    let session = owner.logical_session().clone();
    let filename = session.log_filename(false);
    let mut state = WriterState::<PanickingFlushWriter>::default();
    state.register_owner(owner.clone());
    state.write_contained(
      &session,
      Arc::clone(&filename),
      &serde_json::json!({ "action": "BuildStart" }),
      |_| Ok(()),
      |_| Ok(BufWriter::with_capacity(0, PanickingFlushWriter)),
    );

    let error = state.close_session(&owner).expect_err("flush panic should be retained");
    assert_eq!(error.failures().len(), 1);
    assert_eq!(error.failures()[0].operation(), DevtoolsWriterOperation::FlushFile);
    assert_eq!(error.failures()[0].path(), filename.as_ref());
    assert!(error.failures()[0].to_string().contains("injected flush panic"));
    assert_session_clean(&state, &session);
    assert!(state.files.is_empty());

    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn writer_start_failure_is_returned_without_unwinding() {
    let backend = LazyLock::new(|| {
      WriterBackend::Failed(DevtoolsWriterFailure::message(
        DevtoolsWriterOperation::StartWriter,
        "rolldown-devtools-writer".into(),
        "injected startup failure",
      ))
    });
    let error = flush_session_with_backend(session_key(temp_path("startup"), "session"), &backend)
      .recv()
      .expect("receive immediate startup failure")
      .expect_err("startup should fail");
    assert_eq!(error.failures()[0].operation(), DevtoolsWriterOperation::StartWriter);
  }

  #[test]
  fn synchronous_backend_serializes_writes_and_acknowledges_flush_inline() {
    let cwd = temp_path("synchronous-backend");
    fs::create_dir_all(&cwd).expect("create cwd");
    let owner = session_key(&cwd, "session");
    let session = owner.logical_session().clone();
    let filename = session.log_filename(false);
    let backend = LazyLock::new(|| WriterBackend::Synchronous(Mutex::new(WriterState::default())));

    send_best_effort_to(&backend, LogCommand::RegisterSessionOwner { session: owner.clone() });
    send_best_effort_to(
      &backend,
      LogCommand::Write {
        session,
        filename: Arc::clone(&filename),
        action_value: serde_json::json!({ "action": "BuildStart" }),
      },
    );

    flush_session_with_backend(owner, &backend)
      .recv()
      .expect("receive inline flush result")
      .expect("flush synchronous writer");
    let events = fs::read_to_string(filename.as_ref()).expect("read flushed log");
    assert_eq!(
      serde_json::from_str::<serde_json::Value>(events.trim()).expect("valid JSON")["action"],
      "BuildStart"
    );

    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn canonical_root_and_session_component_boundaries_are_portable() {
    let root = temp_path("canonical-root");
    let project = root.join("project");
    fs::create_dir_all(&project).expect("create project");
    fs::create_dir_all(root.join("other")).expect("create dot-dot source directory");
    let owner = session_key(root.join("other").join("..").join("project"), "requested-session");
    let expected_root =
      fs::canonicalize(&project).expect("canonical project").join("node_modules/.rolldown");

    assert_eq!(Path::new(owner.output_root()), expected_root);
    assert_eq!(
      Path::new(owner.logical_session().log_filename(false).as_ref()),
      expected_root.join("requested-session/logs.json")
    );
    assert_eq!(safe_session_path_component(""), "~");
    assert_eq!(
      safe_session_path_component("Requested-Session"),
      "~5265717565737465642d53657373696f6e"
    );
    assert_eq!(safe_session_path_component("é"), "~c3a9");
    for reserved in [
      "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
      "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
    ] {
      assert!(safe_session_path_component(reserved).starts_with('~'));
    }
    assert_eq!(safe_session_path_component(&"x".repeat(95)).len(), 95);
    assert_eq!(safe_session_path_component(&"X".repeat(95)).len(), 191);
    assert!(safe_session_path_component(&"X".repeat(96)).starts_with("~h"));
    assert_eq!(safe_session_path_component(&"x".repeat(200)).len(), 200);
    assert!(safe_session_path_component(&"x".repeat(201)).starts_with("~h"));

    fs::remove_dir_all(root).expect("remove test root");
  }

  #[cfg(unix)]
  #[test]
  fn canonical_root_unifies_symlink_and_symlink_parent_aliases() {
    use std::os::unix::fs::symlink;

    let root = temp_path("symlink-root");
    let real = root.join("real");
    let nested = real.join("nested");
    fs::create_dir_all(&nested).expect("create real directories");
    let alias = root.join("alias");
    symlink(&real, &alias).expect("create root alias");
    let nested_alias = root.join("nested-alias");
    symlink(&nested, &nested_alias).expect("create nested alias");

    let real_owner = session_key(&real, "session");
    let alias_owner = session_key(&alias, "session");
    assert_eq!(real_owner.logical_session(), alias_owner.logical_session());

    let parent_owner = session_key(nested_alias.join(".."), "session");
    assert_eq!(
      Path::new(parent_owner.output_root()),
      fs::canonicalize(&real).expect("canonical real directory").join("node_modules/.rolldown")
    );

    fs::remove_dir_all(root).expect("remove test root");
  }

  #[test]
  fn every_file_emits_its_own_string_refs_as_valid_json_lines() {
    let large_a = "a".repeat(12 * 1024);
    let large_b = "b".repeat(12 * 1024);
    let action = serde_json::json!({
      "action": "HookTransformCallEnd",
      "content": large_a,
      "module_id": large_b,
    });
    let mut meta = Vec::new();
    let mut logs = Vec::new();
    let mut meta_hashes = FxHashSet::default();
    let mut log_hashes = FxHashSet::default();

    write_event(&mut meta, &action, &mut meta_hashes).expect("write meta");
    write_event(&mut logs, &action, &mut log_hashes).expect("write logs");

    for output in [meta, logs] {
      let events = String::from_utf8(output)
        .expect("utf8 output")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON line"))
        .collect::<Vec<_>>();
      assert_eq!(events.len(), 3);
      assert_eq!(events[0]["action"], "StringRef");
      assert_eq!(events[1]["action"], "StringRef");
      let refs =
        events[..2].iter().map(|event| event["id"].as_str().unwrap()).collect::<FxHashSet<_>>();
      for key in ["content", "module_id"] {
        let hash = events[2][key].as_str().unwrap().trim_start_matches("$ref:");
        assert!(refs.contains(hash));
      }
    }
  }

  #[test]
  fn structural_identity_fields_are_never_rewritten_as_string_refs() {
    let session_id = "s".repeat(12 * 1024);
    let build_id = "b".repeat(12 * 1024);
    let content = "c".repeat(12 * 1024);
    let action = serde_json::json!({
      "action": "HookTransformCallEnd",
      "build_id": build_id,
      "content": content,
      "session_id": session_id,
    });
    let mut output = Vec::new();
    let mut hashes = FxHashSet::default();

    write_event(&mut output, &action, &mut hashes).expect("write action");

    let events = String::from_utf8(output)
      .expect("utf8 output")
      .lines()
      .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("valid JSON line"))
      .collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["action"], "StringRef");
    assert_eq!(events[1]["session_id"], session_id);
    assert_eq!(events[1]["build_id"], build_id);
    assert!(events[1]["content"].as_str().is_some_and(|value| value.starts_with("$ref:")));
  }

  #[test]
  fn open_failure_is_acknowledged_and_owner_state_is_cleaned() {
    let root = temp_path("open-error");
    let filename = root.join("logs.json");
    fs::create_dir_all(&filename).expect("create directory at log filename");

    let owner = session_key(&root, "open-error-session");
    let session = owner.logical_session().clone();
    let filename: Arc<str> = filename.to_string_lossy().into_owned().into();
    let mut state = WriterState::default();
    state.register_owner(owner.clone());
    state.handle(LogCommand::Write {
      session: session.clone(),
      filename: Arc::clone(&filename),
      action_value: serde_json::json!({ "action": "BuildStart" }),
    });
    let error = state.close_session(&owner).expect_err("open should fail");

    assert_eq!(error.failures().len(), 1);
    assert_eq!(error.failures()[0].operation(), DevtoolsWriterOperation::OpenFile);
    assert_eq!(error.failures()[0].path(), filename.as_ref());
    assert_session_clean(&state, &session);

    fs::remove_dir_all(root).expect("remove test directory");
  }

  #[test]
  fn directory_write_and_flush_failures_are_aggregated_before_cleanup() {
    let cwd = temp_path("aggregated-errors");
    fs::create_dir_all(&cwd).expect("create cwd");
    let owner = session_key(&cwd, "aggregated-error-session");
    let session = owner.logical_session().clone();
    let write_filename: Arc<str> = "session/write-error.json".into();
    let flush_filename: Arc<str> = "session/flush-error.json".into();
    let mut state = WriterState::<TestWriter>::default();
    state.register_owner(owner.clone());

    state.write_contained(
      &session,
      Arc::clone(&write_filename),
      &serde_json::json!({ "action": "BuildStart" }),
      |_| Err(io::Error::other("injected directory failure")),
      |_| Ok(BufWriter::with_capacity(0, TestWriter { fail_write: true, fail_flush: false })),
    );
    state.write_contained(
      &session,
      Arc::clone(&flush_filename),
      &serde_json::json!({ "action": "BuildEnd" }),
      |_| Ok(()),
      |_| Ok(BufWriter::with_capacity(0, TestWriter { fail_write: false, fail_flush: true })),
    );

    let error = state.close_session(&owner).expect_err("writer phases should fail");
    let operations =
      error.failures().iter().map(DevtoolsWriterFailure::operation).collect::<Vec<_>>();
    assert_eq!(
      operations,
      [
        DevtoolsWriterOperation::CreateDirectory,
        DevtoolsWriterOperation::WriteEvent,
        DevtoolsWriterOperation::FlushFile,
      ]
    );
    assert_eq!(error.failures()[0].path(), "session");
    assert_eq!(error.failures()[1].path(), write_filename.as_ref());
    assert_eq!(error.failures()[2].path(), flush_filename.as_ref());
    assert_session_clean(&state, &session);
    assert!(state.files.is_empty());

    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn same_logical_session_owners_receive_failures_independently() {
    let cwd = temp_path("same-owner-root");
    fs::create_dir_all(&cwd).expect("create cwd");
    let first = session_key(&cwd, "shared-session");
    let second = session_key(&cwd, "shared-session");
    let session = first.logical_session().clone();
    assert_eq!(session, *second.logical_session());
    assert_ne!(first, second);

    let filename: Arc<str> = session.log_filename(false);
    let mut state = WriterState::<TestWriter>::default();
    state.register_owner(first.clone());
    state.register_owner(second.clone());
    state.write_contained(
      &session,
      Arc::clone(&filename),
      &serde_json::json!({ "action": "BuildStart" }),
      |_| Ok(()),
      |_| Ok(BufWriter::with_capacity(0, TestWriter { fail_write: true, fail_flush: false })),
    );

    let first_error = state.close_session(&first).expect_err("first owner should retain failure");
    assert_eq!(first_error.failures()[0].operation(), DevtoolsWriterOperation::WriteEvent);
    assert!(state.files.contains_key(&filename));
    assert_eq!(state.owners_by_session[&session].len(), 1);
    assert!(state.close_session(&first).is_ok(), "duplicate close must be a no-op");

    let second_error =
      state.close_session(&second).expect_err("second owner should retain failure");
    assert_eq!(second_error.failures()[0].operation(), DevtoolsWriterOperation::WriteEvent);
    assert_session_clean(&state, &session);
    assert!(state.files.is_empty());

    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn late_owner_inherits_retained_failures_without_consuming_existing_owner() {
    let cwd = temp_path("late-owner");
    fs::create_dir_all(&cwd).expect("create cwd");
    let first = session_key(&cwd, "shared-session");
    let session = first.logical_session().clone();
    let filename: Arc<str> = session.log_filename(false);
    let mut state = WriterState::<TestWriter>::default();
    state.register_owner(first.clone());
    state.write_contained(
      &session,
      filename,
      &serde_json::json!({ "action": "BuildStart" }),
      |_| Err(io::Error::other("injected directory failure")),
      |_| Ok(BufWriter::new(TestWriter { fail_write: false, fail_flush: false })),
    );
    let second = session_key(&cwd, "shared-session");
    state.register_owner(second.clone());

    assert!(state.close_session(&first).is_err());
    assert!(state.close_session(&second).is_err());
    assert_session_clean(&state, &session);
    fs::remove_dir_all(cwd).expect("remove cwd");
  }

  #[test]
  fn same_session_id_in_different_output_roots_has_independent_state() {
    let root = temp_path("independent-roots");
    let first_cwd = root.join("first");
    let second_cwd = root.join("second");
    fs::create_dir_all(&first_cwd).expect("create first cwd");
    fs::create_dir_all(&second_cwd).expect("create second cwd");
    let first = session_key(&first_cwd, "shared-session");
    let second = session_key(&second_cwd, "shared-session");
    let directory_attempts = Cell::new(0);
    let mut state = WriterState::<TestWriter>::default();
    state.register_owner(first.clone());
    state.register_owner(second.clone());

    for owner in [&first, &second] {
      state.write_contained(
        owner.logical_session(),
        owner.logical_session().log_filename(false),
        &serde_json::json!({ "action": "BuildStart" }),
        |_| {
          directory_attempts.set(directory_attempts.get() + 1);
          Ok(())
        },
        |_| Ok(BufWriter::new(TestWriter { fail_write: false, fail_flush: false })),
      );
    }

    assert_eq!(directory_attempts.get(), 2);
    state.close_session(&first).expect("first owner should close");
    assert_session_clean(&state, first.logical_session());
    assert!(state.owners_by_session.contains_key(second.logical_session()));
    state.close_session(&second).expect("second owner should close");
    assert_session_clean(&state, second.logical_session());

    fs::remove_dir_all(root).expect("remove root");
  }

  fn assert_session_clean<W: Write>(state: &WriterState<W>, session: &DevtoolsLogicalSessionKey) {
    assert!(!state.files_by_session.contains_key(session));
    assert!(!state.dir_ensured.contains(session));
    assert!(!state.owners_by_session.contains_key(session));
    assert!(!state.failures_by_session.contains_key(session));
    assert!(!state.failures_by_owner.keys().any(|owner| owner.logical_session() == session));
  }
}
