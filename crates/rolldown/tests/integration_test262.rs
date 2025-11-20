#![allow(clippy::ignore_without_reason)]

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::LazyLock;

use rolldown::BundlerOptions;
use rolldown_error::{BuildDiagnostic, EventKind};
use rolldown_testing::integration_test::IntegrationTest;
use rolldown_testing::test_config::TestMeta;
use serde::{Deserialize, Serialize};
use sugar_path::SugarPath;

#[derive(Debug, Deserialize, Serialize)]
struct Test262Negative {
  phase: String,
  #[serde(rename = "type")]
  error_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct Test262Frontmatter {
  description: Option<String>,
  negative: Option<Test262Negative>,
  #[serde(default)]
  flags: Vec<String>,
  #[serde(default)]
  includes: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Test262FailureMetadata {
  reason: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  issue: Option<String>,
}

/// Load failure metadata from JSON file
fn load_failure_metadata() -> HashMap<String, Test262FailureMetadata> {
  let json_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/test262_failures.json");

  let json_content =
    std::fs::read_to_string(json_path).expect("Failed to read test262_failures.json");

  serde_json::from_str(&json_content).expect("Failed to parse test262_failures.json")
}

static KNOWN_FAILURES: LazyLock<HashMap<String, Test262FailureMetadata>> =
  LazyLock::new(load_failure_metadata);

/// Parse YAML frontmatter from a test262 file
fn parse_frontmatter(content: &str) -> Option<Test262Frontmatter> {
  // Find the YAML frontmatter between /*--- and ---*/
  let start = content.find("/*---")?;
  let end = content[start..].find("---*/")?;
  let yaml_content = &content[start + 5..start + end];

  serde_yaml::from_str(yaml_content).ok()
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

#[derive(Debug)]
enum TestOutcome {
  Success(String),
  RuntimeError(String),
  BundleError(Vec<BuildDiagnostic>),
}

impl TestResult {
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
}

#[tokio::test(flavor = "multi_thread")]
#[expect(clippy::too_many_lines)]
async fn test262_module_code() {
  let cwd = std::env::current_dir().unwrap();
  let test262_root = cwd.join("../../test262");
  let module_code_dir = test262_root.join("test/language/module-code");

  assert!(
    module_code_dir.exists(),
    "test262 module-code directory not found at {module_code_dir:?}. \
    Current directory is {cwd:?}. \
    Please ensure test262 is cloned as a submodule."
  );

  let mut test_files = Vec::new();

  // Discover all .js test files, excluding _FIXTURE files
  for entry in
    walkdir::WalkDir::new(&module_code_dir).into_iter().filter_map(Result::ok).filter(|e| {
      e.path().extension().is_some_and(|ext| ext == "js")
        && !e.path().to_string_lossy().contains("_FIXTURE")
    })
  {
    test_files.push(entry.path().to_path_buf());
  }

  test_files.sort();

  // Run tests in parallel using tokio::spawn
  let tasks: Vec<_> = test_files
    .into_iter()
    .map(|test_file| {
      let cwd = cwd.clone();
      let test262_root = test262_root.clone();

      tokio::spawn(async move {
        let content = std::fs::read_to_string(&test_file).unwrap();
        let frontmatter = parse_frontmatter(&content);

        let expected = if let Some(fm) = &frontmatter {
          if let Some(negative) = &fm.negative {
            TestExpectation::Error {
              error_type: negative.error_type.clone(),
              phase: negative.phase.clone(),
            }
          } else {
            TestExpectation::Success
          }
        } else {
          TestExpectation::Success
        };

        // Create a bundler options for this test
        let test_folder = test_file.parent().unwrap();
        let options = BundlerOptions {
          input: Some(vec![rolldown::InputItem {
            name: Some("main".to_string()),
            import: test_file.to_string_lossy().to_string(),
          }]),
          cwd: Some(test_folder.to_path_buf()),
          format: Some(rolldown::OutputFormat::Esm),
          platform: Some(rolldown::Platform::Node),
          keep_names: Some(true),
          generated_code: Some(rolldown::GeneratedCodeOptions { preset: None, symbols: true }),
          ..Default::default()
        };

        let test_meta = TestMeta {
          write_to_disk: false,
          expect_error: frontmatter.as_ref().and_then(|fm| fm.negative.as_ref()).is_some(),
          ..Default::default()
        };

        let integration_test = IntegrationTest::new(test_meta, test_folder.to_path_buf());

        let actual = match integration_test.bundle(options).await {
          Ok(output) => {
            // Check if we should evaluate this test at runtime
            let should_evaluate = match &expected {
              TestExpectation::Success => true,
              TestExpectation::Error { phase, .. } => phase == "runtime",
            };

            if should_evaluate {
              // Write all chunks to a unique subdirectory to avoid conflicts when running in parallel
              // Generate a unique identifier for this test
              let unique_id = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();

              let temp_base = std::env::temp_dir().join("rolldown_test262");
              let temp_dir = temp_base.join(unique_id.to_string());
              std::fs::create_dir_all(&temp_dir).ok();

              // Write package.json to enable ESM
              let package_json = r#"{"type":"module"}"#;
              std::fs::write(temp_dir.join("package.json"), package_json).ok();

              // Check if this is an async test (needed early for wrapper generation)
              let is_async = frontmatter
                .as_ref()
                .map(|fm| fm.flags.contains(&"async".to_string()))
                .unwrap_or(false);

              // Write all chunks to temp directory (including dependencies)
              let mut main_chunk_filename: Option<&str> = None;

              for asset in &output.assets {
                if let rolldown_common::Output::Chunk(chunk) = asset {
                  let chunk_path = temp_dir.join(chunk.filename.as_str());
                  std::fs::write(&chunk_path, &chunk.code).ok();

                  // Track the main chunk (entry point)
                  if chunk.is_entry {
                    main_chunk_filename = Some(chunk.filename.as_str());
                  }
                }
              }

              if let Some(filename) = main_chunk_filename {
                // Create a wrapper file that sets up the test harness and then imports the bundled code
                let wrapper_file = temp_dir.join("__test_wrapper__.mjs");
                // Load test262 harness files
                let mut final_code = String::new();
                let harness_dir = test262_root.join("harness");

                // Always load core harness files
                for core_file in ["assert.js", "sta.js"] {
                  let harness_file = harness_dir.join(core_file);
                  if let Ok(harness_content) = std::fs::read_to_string(&harness_file) {
                    final_code.push_str(&harness_content);
                    final_code.push('\n');
                  }
                }

                // Make harness functions available globally for ESM modules
                final_code.push_str(
                  r"
// Expose harness functions to globalThis for ESM module access
globalThis.assert = assert;
globalThis.Test262Error = Test262Error;
",
                );

                // For async tests, provide Node.js-compatible print() and load doneprintHandle.js
                if is_async {
                  // Provide print() function for Node.js compatibility
                  final_code.push_str(
                    r"
// Node.js-compatible print() function for test262 harness
function print(msg) {
  console.log(msg);
}
",
                  );
                  final_code.push('\n');

                  // Load doneprintHandle.js which defines $DONE
                  let done_harness = harness_dir.join("doneprintHandle.js");
                  if let Ok(done_content) = std::fs::read_to_string(&done_harness) {
                    final_code.push_str(&done_content);
                    final_code.push('\n');
                  }

                  // Ensure $DONE is on globalThis for asyncHelpers.js
                  final_code.push_str("globalThis.$DONE = $DONE;\n");
                }

                // Load additional includes specified in frontmatter
                // These must be loaded after $DONE is defined for async tests
                if let Some(fm) = &frontmatter {
                  for include in &fm.includes {
                    let harness_file = harness_dir.join(include);
                    if let Ok(harness_content) = std::fs::read_to_string(&harness_file) {
                      final_code.push_str(&harness_content);
                      final_code.push('\n');

                      // Extract function names from the harness file and expose them to globalThis
                      // This allows bundled modules to access these functions
                      if let Some(defines_match) = harness_content.lines()
                        .find(|line| line.contains("defines:"))
                      {
                        // Parse the defines array, e.g., "defines: [fnGlobalObject]"
                        if let Some(start) = defines_match.find('[') {
                          if let Some(end) = defines_match.find(']') {
                            let defines_str = &defines_match[start + 1..end];
                            for func_name in defines_str.split(',') {
                              let func_name = func_name.trim();
                              if !func_name.is_empty() {
                                final_code.push_str(&format!("globalThis.{func_name} = {func_name};\n"));
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }

                // For async tests, add $DONE handling and timeout, then dynamically import the bundled code
                if is_async {
                  final_code.push_str(
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
                  final_code.push_str(&format!("import('./{filename}').catch((error) => {{\n"));
                  final_code.push_str(
                    r"  console.error('Failed to import test module:', error);
  process.exit(1);
});
",
                  );
                } else {
                  // For non-async tests, also use dynamic import to ensure global scope is set up first
                  final_code.push_str(&format!("import('./{filename}').catch((error) => {{\n"));
                  final_code.push_str(
                    r"  console.error('Failed to import test module:', error);
  process.exit(1);
});
",
                  );
                }

                // Write the wrapper file
                if let Err(e) = std::fs::write(&wrapper_file, &final_code) {
                  TestOutcome::RuntimeError(format!("Failed to write wrapper file: {e}"))
                } else {
                  // Execute with Node.js using the wrapper file
                  match std::process::Command::new("node").arg(&wrapper_file).output() {
                    Ok(node_output) => {
                      // Clean up the temporary directory for this test
                      std::fs::remove_dir_all(&temp_dir).ok();

                      let stdout = String::from_utf8_lossy(&node_output.stdout);
                      let stderr = String::from_utf8_lossy(&node_output.stderr);

                      // Normalize paths in output to make snapshots stable
                      // Replace the unique temp directory with a placeholder
                      let normalize_path = |text: &str| -> String {
                        let pattern = format!("/tmp/rolldown_test262/{unique_id}/");
                        text.replace(&pattern, "<temp>/")
                      };

                      let normalized_stdout = normalize_path(&stdout);
                      let normalized_stderr = normalize_path(&stderr);

                      // Check for async test completion markers
                      if is_async {
                        if normalized_stdout.contains("Test262:AsyncTestComplete") {
                          TestOutcome::Success("Async test completed successfully".to_string())
                        } else if normalized_stdout.contains("Test262:AsyncTestFailure")
                          || normalized_stderr.contains("Test262:AsyncTestFailure")
                        {
                          let failure_msg =
                            if normalized_stdout.contains("Test262:AsyncTestFailure") {
                              normalized_stdout
                                .lines()
                                .find(|l| l.contains("Test262:AsyncTestFailure"))
                                .unwrap_or("")
                            } else {
                              normalized_stderr
                                .lines()
                                .find(|l| l.contains("Test262:AsyncTestFailure"))
                                .unwrap_or("")
                            };
                          TestOutcome::RuntimeError(format!(
                            "Async test failed: {}",
                            failure_msg.replace("Test262:AsyncTestFailure:", "")
                          ))
                        } else if !normalized_stderr.is_empty() {
                          TestOutcome::RuntimeError(format!(
                            "Runtime error: {}",
                            normalized_stderr.trim()
                          ))
                        } else {
                          TestOutcome::RuntimeError("Async test did not call $DONE".to_string())
                        }
                      } else {
                        // Synchronous test
                        if node_output.status.success() {
                          TestOutcome::Success("Runtime execution succeeded".to_string())
                        } else {
                          TestOutcome::RuntimeError(format!(
                            "Runtime error: {}",
                            normalized_stderr.trim()
                          ))
                        }
                      }
                    }
                    Err(e) => TestOutcome::RuntimeError(format!("Failed to execute Node.js: {e}")),
                  }
                }
              } else {
                TestOutcome::RuntimeError("No chunk found in output".to_string())
              }
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
            // Extract diagnostics and treat AmbiguousExternalNamespaceError warnings as errors
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

        // Look up failure metadata
        let relative_path = test_file.relative(&cwd);
        let display_path = relative_path.to_slash_lossy();
        let clean_path = display_path.strip_prefix("../../test262/test/").unwrap_or(&display_path);

        let (reason, issue) = if let Some(metadata) = KNOWN_FAILURES.get(clean_path) {
          (Some(metadata.reason.clone()), metadata.issue.clone())
        } else {
          (None, None)
        };

        TestResult { path: relative_path, expected, actual, reason, issue }
      })
    })
    .collect();

  // Wait for all tasks to complete concurrently
  let results: Vec<TestResult> =
    futures::future::join_all(tasks).await.into_iter().map(|r| r.unwrap()).collect();

  // Generate snapshot
  let mut snapshot = String::new();

  let passed = results.iter().filter(|r| r.is_pass()).count();
  let failed = results.iter().filter(|r| !r.is_pass()).count();
  let total = results.len();

  writeln!(snapshot, "Summary: {passed} passed, {failed} failed, {total} total\n").unwrap();
  writeln!(snapshot, "---\n").unwrap();

  let mut passed_tests_with_failure_reason = Vec::new();

  for result in &results {
    // Strip the "../../test262/test/" prefix for cleaner paths
    let display_path = result.path.to_slash_lossy();
    let clean_path = display_path.strip_prefix("../../test262/test/").unwrap_or(&display_path);
    writeln!(snapshot, "# {clean_path}").unwrap();

    let status = if result.is_pass() { "PASS" } else { "FAIL" };
    writeln!(snapshot, "Result: {status}").unwrap();

    // Add reason if test is failing and has a known reason
    if result.is_pass() {
      if result.reason.is_some() || result.issue.is_some() {
        passed_tests_with_failure_reason.push(clean_path.to_string());
      }
    } else {
      if let Some(reason) = &result.reason {
        writeln!(snapshot, "Reason: {reason}").unwrap();
      }
      if let Some(issue) = &result.issue {
        writeln!(snapshot, "Issue: {issue}").unwrap();
      }
    }

    match &result.expected {
      TestExpectation::Success => {
        writeln!(snapshot, "Expected: success").unwrap();
      }
      TestExpectation::Error { error_type, phase } => {
        writeln!(snapshot, "Expected: error (type: {error_type}, phase: {phase})").unwrap();
      }
    }

    match &result.actual {
      TestOutcome::Success(msg) => {
        writeln!(snapshot, "Actual: {msg}").unwrap();
      }
      TestOutcome::RuntimeError(err) => {
        writeln!(snapshot, "Actual:\n{err}").unwrap();
      }
      TestOutcome::BundleError(diagnostics) => {
        writeln!(snapshot, "Actual:").unwrap();
        for diagnostic in diagnostics {
          writeln!(snapshot, "{diagnostic}").unwrap();
        }
      }
    }

    writeln!(snapshot, "---\n").unwrap();
  }

  assert!(
    passed_tests_with_failure_reason.is_empty(),
    "Passing test should not have a failure reason: {}",
    passed_tests_with_failure_reason.join(", ")
  );

  // Check that all entries in KNOWN_FAILURES correspond to actual test files
  let all_test_paths: std::collections::HashSet<String> = results
    .iter()
    .map(|r| {
      let display_path = r.path.to_slash_lossy();
      display_path.strip_prefix("../../test262/test/").unwrap_or(&display_path).to_string()
    })
    .collect();
  let unknown_failures: Vec<String> = KNOWN_FAILURES
    .keys()
    .filter(|key| !all_test_paths.contains(key.as_str()))
    .map(ToString::to_string)
    .collect();
  assert!(
    unknown_failures.is_empty(),
    "KNOWN_FAILURES contains entries for non-existent test files: {}",
    unknown_failures.join(", ")
  );

  insta::assert_snapshot!(snapshot);
}
