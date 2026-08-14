// Runs as its own integration-test binary (own process) because
// `try_init_tracing` reads env vars and guards itself with a global
// `IS_INITIALIZED` flag, so scenarios must not share a process.

#[test]
fn invalid_rd_log_filter_disables_tracing_instead_of_panicking() {
  // SAFETY: this test binary is single-threaded (one test) and sets the env
  // var before any other code reads it.
  unsafe {
    // `not_a_level` is not a valid tracing level, so `Targets::from_str` fails.
    // Before the fix this was `.unwrap()`ed and panicked.
    std::env::set_var("RD_LOG", "rolldown=not_a_level");
    std::env::remove_var("RD_LOG_OUTPUT");
  }
  let guard = rolldown_tracing::try_init_tracing();
  assert!(guard.is_none(), "invalid filter should disable tracing, not panic");
}
