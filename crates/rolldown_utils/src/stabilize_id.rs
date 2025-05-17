use std::path::Path;

use rolldown_std_utils::PathExt as _;
use sugar_path::SugarPath as _;

pub fn stabilize_id(module_id: &str, cwd: &Path) -> String {
  if module_id.as_path().is_absolute() {
    module_id.relative(cwd).as_path().expect_to_slash()
  } else if module_id.starts_with('\0') {
    // handle virtual modules
    module_id.replace('\0', "\\0")
  } else {
    module_id.to_string()
  }
}

#[test]
fn test_stabilize_id() {
  let cwd = std::env::current_dir().unwrap();
  // absolute path
  assert_eq!(stabilize_id(cwd.join("src").join("main.js").expect_to_str(), &cwd), "src/main.js");
  assert_eq!(
    stabilize_id(cwd.join("..").join("src").join("main.js").expect_to_str(), &cwd),
    "../src/main.js"
  );

  // non-path specifier
  assert_eq!(stabilize_id("fs", &cwd), "fs");
  assert_eq!(
    stabilize_id("https://deno.land/x/oak/mod.ts", &cwd),
    "https://deno.land/x/oak/mod.ts"
  );

  // virtual module
  assert_eq!(stabilize_id("\0foo", &cwd), "\\0foo");
}
