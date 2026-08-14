// Runs as its own integration-test binary (own process) because
// `try_init_tracing` reads env vars and guards itself with a global
// `IS_INITIALIZED` flag, so scenarios must not share a process.

#[test]
fn json_output_mode_falls_back_to_readable_instead_of_panicking() {
  // SAFETY: this test binary is single-threaded (one test) and sets the env
  // vars before any other code reads them.
  unsafe {
    std::env::set_var("RD_LOG", "debug");
    // Before the fix `RD_LOG_OUTPUT=json` hit a `panic!`.
    std::env::set_var("RD_LOG_OUTPUT", "json");
  }
  let guard = rolldown_tracing::try_init_tracing();
  assert!(guard.is_none(), "json output mode falls back to readable output without a guard");
  // The fallback installed a real subscriber.
  assert!(tracing::dispatcher::has_been_set(), "fallback subscriber should be installed");
}
