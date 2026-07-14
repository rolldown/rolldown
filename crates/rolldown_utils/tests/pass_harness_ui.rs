#[test]
fn pass_harness_contract() {
  let tests = trybuild::TestCases::new();

  // This passing case is load-bearing. trybuild uses `cargo build` for the
  // generated project when the suite contains a pass case, so the non-ZST
  // inline const assertion is evaluated during code generation.
  tests.pass("tests/ui/pass_harness/pass_valid.rs");
  tests.compile_fail("tests/ui/pass_harness/fail_*.rs");
}
