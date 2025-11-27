use rolldown_testing::fixture::Fixture;

#[test]
fn issue_7233_chain() {
  Fixture::new(env!("CARGO_MANIFEST_DIR").to_string() + "/tests/rolldown/issues/7233_chain")
    .run_integration_test();
}
