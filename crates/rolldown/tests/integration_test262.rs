use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::LazyLock;

use anyhow::anyhow;
use cow_utils::CowUtils;
use regex::Regex;
use rolldown::BundlerOptions;
use rolldown_error::{BuildDiagnostic, DiagnosticOptions, EventKind};
use rolldown_testing::integration_test::IntegrationTest;
use rolldown_testing::test_config::TestMeta;
use rustc_hash::FxHashSet;
use serde::{Deserialize, Serialize};
use sugar_path::SugarPath;
use walkdir::WalkDir;

// Match paths like /home/user/rolldown/test262/... or /home/runner/work/rolldown/rolldown/test262/... or C:\path\to\rolldown\test262\...
static TEST262_PATH_RE: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"[^\s]+[/\\]test262[/\\]").unwrap());

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Test262Negative {
  phase: String,
  #[serde(rename = "type")]
  error_type: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct Test262Frontmatter {
  description: Option<String>,
  negative: Option<Test262Negative>,
  #[serde(default)]
  flags: FxHashSet<String>,
  #[serde(default)]
  includes: FxHashSet<String>,
}

impl Test262Frontmatter {
  fn is_negative(&self) -> bool {
    self.negative.is_some()
  }
}

impl FromStr for Test262Frontmatter {
  type Err = anyhow::Error;

  fn from_str(content: &str) -> Result<Self, Self::Err> {
    let start = content.find("/*---").ok_or_else(|| anyhow!("Missing frontmatter start"))?;
    let end = content[start..].find("---*/").ok_or_else(|| anyhow!("Missing frontmatter end"))?;
    let yaml_content = &content[start + 5..start + end];
    Ok(serde_yaml::from_str(yaml_content)?)
  }
}

#[derive(Debug, Deserialize, Serialize)]
struct Test262FailureMetadata {
  reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  issue: Option<String>,
}

static KNOWN_FAILURES: LazyLock<HashMap<String, Test262FailureMetadata>> = LazyLock::new(|| {
  let json_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test262_failures.json");
  let json_content =
    std::fs::read_to_string(json_path).expect("Failed to read test262_failures.json");
  serde_json::from_str(&json_content).expect("Failed to parse test262_failures.json")
});

/// Discovers all test files in the test262 module-code directory
fn discover_test_files(module_code_dir: &PathBuf) -> Vec<PathBuf> {
  let mut test_files = WalkDir::new(module_code_dir)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|e| {
      e.path().extension().is_some_and(|ext| ext == "js")
        && !e.path().to_string_lossy().contains("_FIXTURE")
    })
    .map(|entry| entry.path().to_path_buf())
    .collect::<Vec<_>>();
  test_files.sort();
  test_files
}

#[derive(Debug)]
struct TestResult {
  path: PathBuf,
  expected: TestExpectation,
  actual: TestOutcome,
  reason: Option<String>,
  issue: Option<String>,
}

#[derive(Debug)]
enum TestExpectation {
  Success,
  Error { error_type: String, phase: String },
}

impl TestExpectation {
  /// Returns true if the test should be evaluated at runtime
  fn should_evaluate(&self) -> bool {
    match self {
      Self::Success => true,
      Self::Error { phase, .. } => phase == "runtime",
    }
  }
}

impl From<&Test262Frontmatter> for TestExpectation {
  fn from(frontmatter: &Test262Frontmatter) -> Self {
    if let Some(negative) = &frontmatter.negative {
      Self::Error { error_type: negative.error_type.clone(), phase: negative.phase.clone() }
    } else {
      Self::Success
    }
  }
}

#[derive(Debug)]
enum TestOutcome {
  Success(String),
  RuntimeError(String),
  BundleError(Vec<BuildDiagnostic>),
}

impl TestResult {
  /// Creates a TestResult with failure metadata from KNOWN_FAILURES
  fn with_metadata(
    path: &Path,
    expected: TestExpectation,
    actual: TestOutcome,
    cwd: &Path,
  ) -> Self {
    let relative_path = path.relative(cwd);
    let display_path = relative_path.to_slash_lossy();
    let clean_path = Self::clean_path(&display_path);

    let (reason, issue) = if let Some(metadata) = KNOWN_FAILURES.get(clean_path) {
      (Some(metadata.reason.clone()), metadata.issue.clone())
    } else {
      (None, None)
    };

    Self { path: relative_path, expected, actual, reason, issue }
  }

  fn is_pass(&self) -> bool {
    match (&self.expected, &self.actual) {
      (TestExpectation::Success, TestOutcome::Success(_)) => true,
      (TestExpectation::Error { error_type, phase }, TestOutcome::BundleError(diagnostics)) => {
        // For parse phase errors, any parse failure is considered a SyntaxError match
        if phase == "parse"
          && error_type == "SyntaxError"
          && diagnostics.iter().any(|e| matches!(e.kind(), EventKind::ParseError))
        {
          return true;
        }
        // For resolution phase errors, check for specific error patterns
        if phase == "resolution" {
          return !diagnostics.is_empty();
        }
        // Otherwise, check if the error contains the expected error type
        let error_text = diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n");
        error_text.contains(error_type)
      }
      (TestExpectation::Error { error_type, phase }, TestOutcome::RuntimeError(actual_error)) => {
        // For runtime errors, check if the error message contains the expected error type
        if phase == "runtime" {
          // Runtime errors should contain either the error type name or be a runtime error
          return actual_error.contains(error_type) || actual_error.contains("Runtime error");
        }
        false
      }
      _ => false,
    }
  }

  fn get_clean_path(&self) -> String {
    Self::clean_path(&self.path.to_slash_lossy()).to_string()
  }

  /// Extracts clean path relative to test262/test directory
  fn clean_path(path: &str) -> &str {
    path.strip_prefix("../../test262/test/").unwrap_or(path)
  }
}

/// Creates bundler options for a test262 test file
fn create_bundler_options(test_file: &Path, test_folder: &Path) -> BundlerOptions {
  BundlerOptions {
    input: Some(vec![rolldown::InputItem {
      name: Some("main".to_string()),
      import: test_file.to_string_lossy().to_string(),
    }]),
    cwd: Some(test_folder.to_path_buf()),
    format: Some(rolldown::OutputFormat::Esm),
    platform: Some(rolldown::Platform::Node),
    keep_names: Some(true),
    generated_code: Some(rolldown::GeneratedCodeOptions { symbols: true }),
    ..Default::default()
  }
}

/// Handles runtime test evaluation
struct RuntimeEvaluator {
  test262_root: PathBuf,
  frontmatter: Test262Frontmatter,
}

impl RuntimeEvaluator {
  fn new(test262_root: PathBuf, frontmatter: Test262Frontmatter) -> Self {
    Self { test262_root, frontmatter }
  }

  fn is_async(&self) -> bool {
    self.frontmatter.flags.contains("async")
  }

  /// Builds the test harness code with all required includes
  fn build_harness_code(&self) -> String {
    let mut code = String::new();
    let harness_dir = self.test262_root.join("harness");

    // Load core harness files
    for core_file in ["assert.js", "sta.js"] {
      if let Ok(content) = std::fs::read_to_string(harness_dir.join(core_file)) {
        code.push_str(&content);
        code.push('\n');
      }
    }

    // Expose harness functions globally for ESM modules
    code.push_str(
      r"
// Expose harness functions to globalThis for ESM module access
globalThis.assert = assert;
globalThis.Test262Error = Test262Error;
",
    );

    if self.is_async() {
      Self::add_async_setup(&mut code, &harness_dir);
    }
    self.add_includes(&mut code, &harness_dir);
    code
  }

  fn add_async_setup(code: &mut String, harness_dir: &Path) {
    code.push_str(
      r"
// Node.js-compatible print() function for test262 harness
function print(msg) {
  console.log(msg);
}
",
    );
    code.push('\n');

    // Load doneprintHandle.js which defines $DONE
    if let Ok(done_content) = std::fs::read_to_string(harness_dir.join("doneprintHandle.js")) {
      code.push_str(&done_content);
      code.push('\n');
    }

    code.push_str("globalThis.$DONE = $DONE;\n");
  }

  fn add_includes(&self, code: &mut String, harness_dir: &Path) {
    for include in &self.frontmatter.includes {
      if let Ok(harness_content) = std::fs::read_to_string(harness_dir.join(include)) {
        code.push_str(&harness_content);
        code.push('\n');

        // Extract and expose function names to globalThis
        if let Some(defines_match) = harness_content.lines().find(|line| line.contains("defines:"))
        {
          if let Some(start) = defines_match.find('[') {
            if let Some(end) = defines_match.find(']') {
              let defines_str = &defines_match[start + 1..end];
              for func_name in defines_str.split(',') {
                let func_name = func_name.trim();
                if !func_name.is_empty() {
                  writeln!(code, "globalThis.{func_name} = {func_name};").unwrap();
                }
              }
            }
          }
        }
      }
    }
  }

  fn build_wrapper_code(&self, filename: &str) -> String {
    let mut code = self.build_harness_code();

    if self.is_async() {
      code.push_str(
        r"

// Track if $DONE was called and timeout handle
let __$DONECalled = false;
const __timeoutHandle = setTimeout(() => {
  if (!__$DONECalled) {
    console.error('Test262:AsyncTestFailure:Test262Error: $DONE was not called');
    process.exit(1);
  }
}, 5000);

// Wrap $DONE to exit the process after printing results
const __original$DONE = $DONE;
globalThis.$DONE = function(error) {
  __$DONECalled = true;
  clearTimeout(__timeoutHandle);
  __original$DONE(error);
  // Give a tiny delay for console.log to flush
  setTimeout(() => process.exit(error ? 1 : 0), 10);
};

// Now import the bundled test module
",
      );
    }

    let _ = write!(
      code,
      "import('./{filename}').catch((error) => {{\n  \
       console.error('Failed to import test module:', error);\n  \
       process.exit(1);\n\
       }});\n"
    );

    code
  }

  /// Evaluates a bundled test by running it with Node.js
  fn evaluate(&self, test_file: &Path, output: &rolldown::BundleOutput) -> TestOutcome {
    let unique_id = {
      use std::hash::{DefaultHasher, Hash, Hasher};
      let mut hasher = DefaultHasher::new();
      test_file.to_string_lossy().hash(&mut hasher);
      hasher.finish().to_string()
    };
    let temp_dir = std::env::temp_dir().join("rolldown_test262").join(&unique_id);

    if let Err(e) = std::fs::create_dir_all(&temp_dir) {
      return TestOutcome::RuntimeError(format!("Failed to create temp dir: {e}"));
    }
    if let Err(e) = std::fs::write(temp_dir.join("package.json"), r#"{"type":"module"}"#) {
      return TestOutcome::RuntimeError(format!("Failed to write package.json: {e}"));
    }

    let mut main_chunk_filename: Option<String> = None;
    for asset in &output.assets {
      if let rolldown_common::Output::Chunk(chunk) = asset {
        let chunk_path = temp_dir.join(chunk.filename.as_str());
        if let Err(e) = std::fs::write(&chunk_path, &chunk.code) {
          return TestOutcome::RuntimeError(format!("Failed to write chunk: {e}"));
        }
        if chunk.is_entry {
          main_chunk_filename = Some(chunk.filename.to_string());
        }
      }
    }
    let Some(filename) = main_chunk_filename else {
      return TestOutcome::RuntimeError("No entry chunk found in output".to_string());
    };

    let wrapper_file = temp_dir.join("__test_wrapper__.mjs");
    let wrapper_code = self.build_wrapper_code(&filename);
    if let Err(e) = std::fs::write(&wrapper_file, &wrapper_code) {
      return TestOutcome::RuntimeError(format!("Failed to write wrapper file: {e}"));
    }

    match std::process::Command::new("node").arg(&wrapper_file).output() {
      Ok(node_output) => {
        _ = std::fs::remove_dir_all(&temp_dir);
        self.process_node_output(&node_output, &temp_dir)
      }
      Err(e) => TestOutcome::RuntimeError(format!("Failed to execute Node.js: {e}")),
    }
  }

  fn process_node_output(
    &self,
    node_output: &std::process::Output,
    temp_dir: &Path,
  ) -> TestOutcome {
    let stdout = String::from_utf8_lossy(&node_output.stdout);
    let stderr = String::from_utf8_lossy(&node_output.stderr);

    let normalized_stdout = Self::normalize_output(&stdout, temp_dir);
    let normalized_stderr = Self::normalize_output(&stderr, temp_dir);

    if self.is_async() {
      Self::process_async_output(&normalized_stdout, &normalized_stderr)
    } else {
      Self::process_sync_output(node_output.status.success(), &normalized_stderr)
    }
  }

  /// Normalizes output to remove machine-dependent information.
  /// - Replaces temp directory paths with `<temp>/`
  /// - Removes stack trace lines (lines starting with "at ")
  fn normalize_output(output: &str, temp_dir: &Path) -> String {
    // Replace temp directory path with <temp>/
    let pattern = format!("{}/", temp_dir.to_string_lossy());
    let output = output.replace(&pattern, "<temp>/");

    // Remove stack trace lines (lines starting with whitespace followed by "at ")
    output
      .lines()
      .filter(|line| !line.trim_start().starts_with("at "))
      .collect::<Vec<_>>()
      .join("\n")
  }

  fn process_async_output(stdout: &str, stderr: &str) -> TestOutcome {
    if stdout.contains("Test262:AsyncTestComplete") {
      TestOutcome::Success("Async test completed successfully".to_string())
    } else if stdout.contains("Test262:AsyncTestFailure")
      || stderr.contains("Test262:AsyncTestFailure")
    {
      let failure_msg = if stdout.contains("Test262:AsyncTestFailure") {
        stdout.lines().find(|l| l.contains("Test262:AsyncTestFailure")).unwrap_or("")
      } else {
        stderr.lines().find(|l| l.contains("Test262:AsyncTestFailure")).unwrap_or("")
      };
      TestOutcome::RuntimeError(format!(
        "Async test failed: {}",
        failure_msg.replace("Test262:AsyncTestFailure:", "")
      ))
    } else if !stderr.is_empty() {
      TestOutcome::RuntimeError(format!("Runtime error: {}", stderr.trim()))
    } else {
      TestOutcome::RuntimeError("Async test did not call $DONE".to_string())
    }
  }

  fn process_sync_output(success: bool, stderr: &str) -> TestOutcome {
    if success {
      TestOutcome::Success("Runtime execution succeeded".to_string())
    } else {
      TestOutcome::RuntimeError(format!("Runtime error: {}", stderr.trim()))
    }
  }
}

/// Normalizes file paths in error messages to be machine-independent.
/// Replaces absolute paths to test262 directory with `<test262>`.
fn normalize_diagnostic_output(output: &str) -> String {
  TEST262_PATH_RE.replace_all(output, "<test262>/").cow_replace('\\', "/").into_owned()
}

/// Generates a snapshot string from test results
fn generate_snapshot(results: &[TestResult]) -> String {
  let mut snapshot = String::new();

  let passed = results.iter().filter(|r| r.is_pass()).count();
  let failed = results.iter().filter(|r| !r.is_pass()).count();
  let total = results.len();
  _ = writeln!(snapshot, "Summary: {passed} passed, {failed} failed, {total} total\n");
  _ = writeln!(snapshot, "---\n");

  for result in results {
    _ = writeln!(snapshot, "# {}", result.get_clean_path());
    _ = writeln!(snapshot, "Result: {}", if result.is_pass() { "PASS" } else { "FAIL" });

    if !result.is_pass() {
      if let Some(reason) = &result.reason {
        _ = writeln!(snapshot, "Reason: {reason}");
      }
      if let Some(issue) = &result.issue {
        _ = writeln!(snapshot, "Issue: {issue}");
      }
    }

    match &result.expected {
      TestExpectation::Success => {
        _ = writeln!(snapshot, "Expected: success");
      }
      TestExpectation::Error { error_type, phase } => {
        _ = writeln!(snapshot, "Expected: error (type: {error_type}, phase: {phase})");
      }
    }

    match &result.actual {
      TestOutcome::Success(msg) => {
        _ = writeln!(snapshot, "Actual: {msg}");
      }
      TestOutcome::RuntimeError(err) => {
        _ = writeln!(snapshot, "Actual:\n{err}");
      }
      TestOutcome::BundleError(diagnostics) => {
        _ = writeln!(snapshot, "Actual:");
        // Sort diagnostics for deterministic output
        let mut normalized_diagnostics: Vec<_> = diagnostics
          .iter()
          .map(|d| {
            normalize_diagnostic_output(&d.to_message_with(&DiagnosticOptions {
              cwd: result.path.parent().unwrap_or(&result.path).to_path_buf(),
            }))
          })
          .collect();
        normalized_diagnostics.sort();
        for normalized in normalized_diagnostics {
          _ = writeln!(snapshot, "{normalized}");
        }
      }
    }
    _ = writeln!(snapshot, "---\n");
  }
  snapshot
}

/// Validates that passing tests don't have failure metadata
fn validate_no_stale_pass_metadata(results: &[TestResult]) {
  let passed_tests_with_failure_reason: Vec<String> = results
    .iter()
    .filter(|r| r.is_pass() && (r.reason.is_some() || r.issue.is_some()))
    .map(TestResult::get_clean_path)
    .collect();

  assert!(
    passed_tests_with_failure_reason.is_empty(),
    "Passing test should not have a failure reason: {}",
    passed_tests_with_failure_reason.join(", ")
  );
}

/// Validates that all KNOWN_FAILURES entries correspond to actual test files
fn validate_known_failures(results: &[TestResult]) {
  let all_test_paths: std::collections::HashSet<String> =
    results.iter().map(TestResult::get_clean_path).collect();

  let unknown_failures: Vec<String> =
    KNOWN_FAILURES.keys().filter(|key| !all_test_paths.contains(key.as_str())).cloned().collect();

  assert!(
    unknown_failures.is_empty(),
    "KNOWN_FAILURES contains entries for non-existent test files: {}",
    unknown_failures.join(", ")
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn test262_module_code() {
  let cwd = std::env::current_dir().unwrap();
  let test262_root = cwd.join("../../test262");
  let module_code_dir = test262_root.join("test/language/module-code");
  assert!(
    module_code_dir.exists(),
    "test262 module-code directory not found at {module_code_dir:?}. \
    Please ensure test262 is cloned as a submodule."
  );

  let filter = std::env::var("TEST262_FILTER").ok();
  let test_files: Vec<_> = discover_test_files(&module_code_dir)
    .into_iter()
    .filter(|test_file| {
      // Example: TEST262_FILTER="export-default" cargo test --test integration_test262 -- --no-capture
      filter.as_ref().is_none_or(|filter| test_file.to_string_lossy().contains(filter))
    })
    .collect();

  if test_files.is_empty() {
    if let Some(ref f) = filter {
      eprintln!("No tests matched filter: {f}");
    }
    return;
  }
  eprintln!("Running {} test(s)", test_files.len());

  let tasks: Vec<_> = test_files
    .into_iter()
    .map(|test_file| {
      let cwd = cwd.clone();
      let test262_root = test262_root.clone();

      tokio::spawn(async move {
        let content = std::fs::read_to_string(&test_file).unwrap();
        let frontmatter = Test262Frontmatter::from_str(&content).unwrap_or_default();
        let expected = TestExpectation::from(&frontmatter);

        let test_folder = test_file.parent().unwrap();
        let options = create_bundler_options(&test_file, test_folder);

        let test_meta = TestMeta {
          write_to_disk: false,
          expect_error: frontmatter.is_negative(),
          ..Default::default()
        };

        let integration_test = IntegrationTest::new(test_meta, test_folder.to_path_buf());
        let actual = match integration_test.bundle(options).await {
          Ok(output) => {
            if expected.should_evaluate() {
              let evaluator = RuntimeEvaluator::new(test262_root.clone(), frontmatter);
              evaluator.evaluate(&test_file, &output)
            } else {
              // For non-runtime tests, just report bundle success
              let chunk_count = output
                .assets
                .iter()
                .filter(|a| matches!(a, rolldown_common::Output::Chunk(_)))
                .count();
              TestOutcome::Success(format!("{chunk_count} chunk(s) generated"))
            }
          }
          Err(err) => {
            // Treat AmbiguousExternalNamespaceError warnings as errors
            let diagnostics = err
              .into_vec()
              .into_iter()
              .filter(|diag| {
                matches!(diag.severity(), rolldown_error::Severity::Error)
                  || matches!(diag.kind(), EventKind::AmbiguousExternalNamespaceError)
              })
              .collect();
            TestOutcome::BundleError(diagnostics)
          }
        };
        Ok(TestResult::with_metadata(&test_file, expected, actual, &cwd))
      })
    })
    .collect();

  let results = futures::future::join_all(tasks)
    .await
    .into_iter()
    .map(|r| r.unwrap())
    .collect::<Result<Vec<TestResult>, anyhow::Error>>()
    .unwrap();

  if filter.is_some() {
    let snapshot = generate_snapshot(&results);
    eprintln!("{snapshot}");
    return;
  }

  validate_no_stale_pass_metadata(&results);
  validate_known_failures(&results);

  let snapshot = generate_snapshot(&results);
  insta::assert_snapshot!(snapshot);
}
