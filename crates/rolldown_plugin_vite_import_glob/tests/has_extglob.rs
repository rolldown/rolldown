use rolldown_plugin_vite_import_glob::utils::GlobImportVisit;

#[test]
fn detects_negation_form() {
  assert!(GlobImportVisit::has_extglob("!(*.d.ts)"));
  assert!(GlobImportVisit::has_extglob("**/!(*.d.ts)"));
  assert!(GlobImportVisit::has_extglob("./routes/**/!(*.d.ts)"));
}

#[test]
fn detects_other_extglob_operators() {
  assert!(GlobImportVisit::has_extglob("?(x)"));
  assert!(GlobImportVisit::has_extglob("*(x)"));
  assert!(GlobImportVisit::has_extglob("+(.js|.ts)"));
  assert!(GlobImportVisit::has_extglob("@(foo|bar)"));
  assert!(GlobImportVisit::has_extglob("[jt]s?(x)"));
  // Note: (x)? is NOT standard extglob — accidental tinyglobby quirk, not detected here
}

#[test]
fn does_not_flag_standard_patterns() {
  assert!(!GlobImportVisit::has_extglob("**/*.ts"));
  assert!(!GlobImportVisit::has_extglob("./**/*.{ts,tsx}"));
  assert!(!GlobImportVisit::has_extglob("./dir/*.js"));
  assert!(!GlobImportVisit::has_extglob("[abc]"));
  // Leading '!' in array negation form — not extglob
  assert!(!GlobImportVisit::has_extglob("!**/*.d.ts"));
  // Directories with '(' in their name must not be flagged
  assert!(!GlobImportVisit::has_extglob("./foo(bar)/*.ts"));
}

#[test]
fn does_not_flag_escaped_chars() {
  assert!(!GlobImportVisit::has_extglob("\\!(*.d.ts)"));
  assert!(!GlobImportVisit::has_extglob("\\*(foo)"));
}
