use std::path::PathBuf;

use rolldown::BundlerOptions;
use rolldown_testing::{fixture::Fixture, integration_test::IntegrationTest};
use rolldown_testing_config::{ExpectedExecutionFailure, TestMeta};

fn fixture(name: &str) -> Fixture {
  Fixture::new(
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
      .join("tests/fixtures/expected_execution_failure")
      .join(name),
  )
}

#[test]
fn accepts_a_matching_expected_execution_failure() {
  fixture("xfail").run_integration_test();
}

#[test]
fn scopes_an_expected_failure_to_one_config_variant() {
  fixture("variant").run_integration_test();
}

#[test]
#[should_panic(expected = "Expected the bundler to be success, but failed to create it")]
fn does_not_hide_bundler_creation_errors() {
  fixture("config_error").run_integration_test();
}

#[test]
fn preserves_bundler_creation_error_snapshots_without_a_runtime_marker() {
  fixture("config_error_without_marker").run_integration_test();
}

#[test]
#[should_panic(expected = "outputContains` entries must not be empty or whitespace-only")]
fn rejects_blank_output_matchers() {
  fixture("blank_matcher").run_integration_test();
}

#[test]
#[should_panic(expected = "reason` must not be empty or whitespace-only")]
fn rejects_a_blank_failure_reason() {
  fixture("blank_reason").run_integration_test();
}

#[test]
#[should_panic(expected = "execution failed for a different reason than expected")]
fn rejects_a_non_matching_execution_failure() {
  fixture("mismatch").run_integration_test();
}

#[test]
#[should_panic(expected = "XPASS: generated output was expected to fail execution")]
fn rejects_an_unexpected_execution_success() {
  fixture("xpass").run_integration_test();
}

#[test]
#[should_panic(expected = "XPASS: generated output was expected to fail execution")]
fn rejects_an_unexpected_success_through_the_manual_api() {
  let meta = TestMeta {
    snapshot: false,
    expect_execution_failure: Some(ExpectedExecutionFailure {
      reason: "exercise manual IntegrationTest XPASS detection".to_string(),
      output_contains: vec!["unused because execution succeeds".to_string()],
    }),
    ..Default::default()
  };

  tokio::runtime::Runtime::new().unwrap().block_on(
    IntegrationTest::new(
      meta,
      PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/expected_execution_failure/xpass_manual"),
    )
    .run(BundlerOptions::default()),
  );
}
