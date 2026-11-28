use std::{
  fs,
  path::{Path, PathBuf},
  process::Stdio,
  sync::{Arc, Mutex},
  time::Duration,
};

use rolldown_common::{ClientHave, HmrNumSet, HmrPatch, HmrPullResponse};
use rolldown_dev::DevEngine;
use rolldown_error::BuildResult;
use serde_json::{Value, json};
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
  process::{Child, ChildStdin, ChildStdout, Command},
  task::JoinHandle,
};

/// Marks the stdout lines of `hmr-client-driver.js` that carry a message.
const MESSAGE_PREFIX: &str = "@@rolldown-hmr-client@@ ";
/// Covers a cold node start on a slow CI machine.
const MESSAGE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a failure message waits for the last stderr bytes of a client that exited.
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// One patch request of the client, with the module sets decoded to sorted ids.
pub struct HmrPullRecord {
  pub generation: u32,
  pub update: Vec<String>,
  pub executed: Vec<String>,
  pub factories: Vec<String>,
  pub response: BuildResult<HmrPullResponse<HmrPatch>>,
}

/// What the client did with one push.
pub struct HmrClientStepRecord {
  pub pulls: Vec<HmrPullRecord>,
  /// `applied`, `noop`, `stale-only`, or `reload: <reason>` when the server answered a pull
  /// with a reload or an error. `push lost` when the test did not send the push.
  pub result: String,
}

impl HmrClientStepRecord {
  pub fn reloads(&self) -> bool {
    self.result.starts_with("reload")
  }
}

/// A node process that runs the output like one client tab: it loads the entries, then handles
/// each push the way the FBM client does, and pulls patches through `DevEngine::render_patch`.
/// See `hmr-client-driver.js` for the other side.
pub struct HmrTestClient {
  child: Child,
  stdin: Option<ChildStdin>,
  stdout: Lines<BufReader<ChildStdout>>,
  /// Fixture logs and protocol lines, for failure messages.
  stdout_log: Vec<String>,
  stderr_log: Arc<Mutex<String>>,
  stderr_reader: Option<JoinHandle<()>>,
  /// A copy of the output the client loaded. Later builds overwrite the output directory, and
  /// a client keeps running what it loaded until it reloads.
  dir: PathBuf,
  pulls: usize,
}

impl HmrTestClient {
  /// `globals_injection` runs before the entries, through `--import`. `output_generation` is the
  /// HMR generation of the output in `output_dir`.
  pub async fn spawn(
    output_dir: &Path,
    entry_file_names: &[String],
    output_generation: u32,
    globals_injection: Option<String>,
  ) -> Self {
    let dir = PathBuf::from(format!("{}-client", output_dir.display()));
    match fs::remove_dir_all(&dir) {
      Ok(()) => {}
      Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
      Err(err) => panic!("failed to clean {}: {err}", dir.display()),
    }
    copy_dir(output_dir, &dir);
    fs::write(dir.join("package.json"), r#"{ "type": "module" }"#).unwrap();

    let entries = entry_file_names
      .iter()
      .map(|name| dir.join(name).to_str().expect("should be valid utf8").to_string())
      .collect::<Vec<_>>();

    let mut command = Command::new("node");
    if let Some(globals_injection) = globals_injection {
      command.arg("--import").arg(globals_injection);
    }
    command
      .arg("--input-type=module")
      .arg("--eval")
      .arg(include_str!("./hmr-client-driver.js"))
      .env("ROLLDOWN_HMR_CLIENT_ENTRIES", serde_json::to_string(&entries).unwrap())
      .env("ROLLDOWN_HMR_CLIENT_GENERATION", output_generation.to_string())
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .kill_on_drop(true);
    let mut child = command.spawn().expect("failed to spawn node");

    // Drained all the time, so a full pipe cannot block node.
    let stderr_log = Arc::new(Mutex::new(String::new()));
    let mut stderr = child.stderr.take().unwrap();
    let stderr_sink = Arc::clone(&stderr_log);
    let stderr_reader = tokio::spawn(async move {
      let mut buf = vec![0; 4096];
      while let Ok(n) = stderr.read(&mut buf).await {
        if n == 0 {
          break;
        }
        stderr_sink.lock().unwrap().push_str(&String::from_utf8_lossy(&buf[..n]));
      }
    });

    let stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let stdin = child.stdin.take();
    let mut client = Self {
      child,
      stdin,
      stdout,
      stdout_log: vec![],
      stderr_log,
      stderr_reader: Some(stderr_reader),
      dir,
      pulls: 0,
    };
    let ready = client.next_message().await;
    client.expect_type(&ready, "ready");
    client
  }

  /// Sends the push of a `Changed` update and serves the client's patch requests until it
  /// reports what it did.
  pub async fn push(
    &mut self,
    dev_engine: &DevEngine,
    changed_ids: &[String],
    stale_ids: &[String],
    generation: u32,
  ) -> HmrClientStepRecord {
    self
      .send(&json!({ "t": "push", "changedIds": changed_ids, "staleIds": stale_ids, "gen": generation }))
      .await;
    let mut pulls = vec![];
    loop {
      let message = self.next_message().await;
      match message["t"].as_str() {
        Some("request") => {
          let query = message["query"].as_str().expect("a request has a query");
          let response = dev_engine.render_patch(query).await;
          let reply = match &response {
            Ok(HmrPullResponse::Payload(patch)) => {
              let file = self.dir.join(format!("__rolldown_hmr_patch_{}.js", self.pulls));
              self.pulls += 1;
              fs::write(&file, &patch.code).unwrap();
              json!({ "t": "patch", "file": file })
            }
            Ok(HmrPullResponse::Reload { reason }) => {
              // The body Vite serves for a reload answer.
              let file = self.dir.join(format!("__rolldown_hmr_patch_{}.js", self.pulls));
              self.pulls += 1;
              let reason = serde_json::to_string(reason).unwrap();
              fs::write(
                &file,
                format!("__rolldown_runtime__.reloadNeeded({reason});\nexport {{}}"),
              )
              .unwrap();
              json!({ "t": "patch", "file": file })
            }
            // Vite answers with an error status, and the client's import fails.
            Err(errs) => json!({ "t": "error", "reason": format!("{errs:?}") }),
          };
          pulls.push(decode_request(dev_engine, query, response).await);
          self.send(&reply).await;
        }
        Some("result") => {
          let status = message["status"].as_str().expect("a result has a status");
          let result = if status == "reload" {
            match &pulls.last().map(|pull| &pull.response) {
              Some(Ok(HmrPullResponse::Reload { reason })) => format!("reload: {reason}"),
              _ => "reload: the patch request failed".to_owned(),
            }
          } else {
            status.to_owned()
          };
          return HmrClientStepRecord { pulls, result };
        }
        _ => self.fail(&format!("unexpected message {message}")),
      }
    }
  }

  /// Ends the client and waits for it to exit, so the fixture's `beforeExit` checks run.
  pub async fn finish(mut self) {
    self.send(&json!({ "t": "done" })).await;
    self.stdin.take();
    // Read the rest of stdout, so the failure message has the fixture logs.
    while let Ok(Ok(Some(line))) =
      tokio::time::timeout(MESSAGE_TIMEOUT, self.stdout.next_line()).await
    {
      self.stdout_log.push(line);
    }
    let status = match tokio::time::timeout(MESSAGE_TIMEOUT, self.child.wait()).await {
      Ok(status) => status.expect("failed to wait for node"),
      Err(_) => self.fail("the client did not exit"),
    };
    if !status.success() {
      self.fail_after_exit(&format!("the client exited with {status}")).await;
    }
  }

  async fn send(&mut self, message: &Value) {
    let stdin = self.stdin.as_mut().expect("stdin is open until `finish`");
    let line = format!("{message}\n");
    if let Err(err) = stdin.write_all(line.as_bytes()).await {
      self.fail(&format!("failed to write to the client: {err}"));
    }
  }

  async fn next_message(&mut self) -> Value {
    loop {
      let line = match tokio::time::timeout(MESSAGE_TIMEOUT, self.stdout.next_line()).await {
        Ok(Ok(Some(line))) => line,
        Ok(Ok(None)) => self.fail_after_exit("the client exited early").await,
        Ok(Err(err)) => self.fail(&format!("failed to read from the client: {err}")),
        Err(_) => self.fail("timed out waiting for the client"),
      };
      let message = line.strip_prefix(MESSAGE_PREFIX).map(serde_json::from_str::<Value>);
      self.stdout_log.push(line);
      match message {
        Some(Ok(message)) => {
          if message["t"] == "error" {
            let error = message["message"].as_str().unwrap_or_default().to_owned();
            self.fail(&format!("the client failed: {error}"));
          }
          return message;
        }
        Some(Err(err)) => self.fail(&format!("bad message from the client: {err}")),
        None => {}
      }
    }
  }

  fn expect_type(&self, message: &Value, expected: &str) {
    if message["t"] != expected {
      self.fail(&format!("expected `{expected}`, got {message}"));
    }
  }

  /// The stderr reader can still hold the last bytes when stdout ends, and those bytes often
  /// have the error that made the client exit.
  async fn fail_after_exit(&mut self, reason: &str) -> ! {
    if let Some(reader) = self.stderr_reader.take() {
      let _ = tokio::time::timeout(STDERR_DRAIN_TIMEOUT, reader).await;
    }
    self.fail(reason)
  }

  fn fail(&self, reason: &str) -> ! {
    let stderr = self.stderr_log.lock().unwrap().clone();
    panic!(
      "HMR test client: {reason}\n⬇️⬇️ stderr ⬇️⬇️\n{stderr}\n⬇️⬇️ stdout ⬇️⬇️\n{}\n⬆️⬆️ end ⬆️⬆️",
      self.stdout_log.join("\n")
    );
  }
}

/// Every module that has a number. Right after a full build, these are the modules of its
/// output.
pub async fn numbered_ids(dev_engine: &DevEngine) -> Vec<String> {
  let bundler = dev_engine.bundler();
  let bundler = bundler.lock().await;
  let session = bundler.hmr_session().lock();
  (0..).map_while(|num| session.id_of(num)).map(ToString::to_string).collect()
}

/// The pull of a page that ran `executed` and holds no factories, for the update of
/// `changed_ids` at `generation`. For a fixture that runs no client.
pub async fn harness_pull(
  dev_engine: &DevEngine,
  changed_ids: &[String],
  generation: u32,
  executed: &[String],
) -> HmrClientStepRecord {
  let query = {
    let bundler = dev_engine.bundler();
    let bundler = bundler.lock().await;
    let session = bundler.hmr_session().lock();
    let nums = |ids: &mut dyn Iterator<Item = &String>| {
      HmrNumSet::from_nums(ids.filter_map(|id| session.num_of(id)))
    };
    ClientHave {
      epoch: session.epoch(),
      generation,
      update: nums(&mut changed_ids.iter()),
      executed: nums(&mut executed.iter().filter(|id| !changed_ids.contains(id))),
      ..ClientHave::default()
    }
    .to_query()
  };
  let response = dev_engine.render_patch(&query).await;
  HmrClientStepRecord {
    pulls: vec![decode_request(dev_engine, &query, response).await],
    result: "not run: the fixture runs no client, so the harness made this pull".to_owned(),
  }
}

async fn decode_request(
  dev_engine: &DevEngine,
  query: &str,
  response: BuildResult<HmrPullResponse<HmrPatch>>,
) -> HmrPullRecord {
  let bundler = dev_engine.bundler();
  let bundler = bundler.lock().await;
  let session = bundler.hmr_session().lock();
  let have =
    ClientHave::parse_query(query, session.num_count()).expect("the runtime builds valid queries");
  let ids = |set: &rolldown_common::HmrNumSet| {
    let mut ids = set
      .iter()
      .map(|num| session.id_of(num).map_or_else(|| format!("<unknown {num}>"), ToString::to_string))
      .collect::<Vec<_>>();
    ids.sort();
    ids
  };
  HmrPullRecord {
    generation: have.generation,
    update: ids(&have.update),
    executed: ids(&have.executed),
    factories: ids(&have.factories),
    response,
  }
}

fn copy_dir(from: &Path, to: &Path) {
  let mut dirs = vec![from.to_path_buf()];
  while let Some(dir) = dirs.pop() {
    let Ok(entries) = fs::read_dir(&dir) else { continue };
    for entry in entries {
      let path = entry.unwrap().path();
      if path.is_dir() {
        dirs.push(path);
      } else {
        let target = to.join(path.strip_prefix(from).unwrap());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(&path, target).unwrap();
      }
    }
  }
}
