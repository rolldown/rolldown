use rolldown_testing::fixture::Fixture;

#[test]
fn issue_7233() {
  Fixture::new(env!("CARGO_MANIFEST_DIR").to_string() + "/tests/rolldown/issues/7233")
    .run_integration_test();
}
