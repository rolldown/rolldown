use std::path::Path;

use rolldown_utils::path_ext::PathExt;
use sugar_path::SugarPath;

pub fn stabilize_resource_id(resource_id: &str, cwd: &Path) -> String {
  if resource_id.as_path().is_absolute() {
    resource_id.relative(cwd).as_path().expect_to_slash()
  } else if resource_id.starts_with('\0') {
    // handle virtual modules
    resource_id.replacen('\0', "\\0", 1)
  } else {
    resource_id.to_string()
  }
}

#[test]
fn test_stabilize_resource_id() {
  let cwd = std::env::current_dir().unwrap();
  // absolute path
  assert_eq!(
    stabilize_resource_id(cwd.join("src").join("main.js").expect_to_str(), &cwd),
    "src/main.js"
  );
  assert_eq!(
    stabilize_resource_id(cwd.join("..").join("src").join("main.js").expect_to_str(), &cwd),
    "../src/main.js"
  );

  // non-path specifier
  assert_eq!(stabilize_resource_id("fs", &cwd), "fs");
  assert_eq!(
    stabilize_resource_id("https://deno.land/x/oak/mod.ts", &cwd),
    "https://deno.land/x/oak/mod.ts"
  );

  // virtual module
  assert_eq!(stabilize_resource_id("\0foo", &cwd), "\\0foo");
}
