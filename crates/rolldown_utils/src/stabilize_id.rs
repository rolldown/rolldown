use std::path::Path;

use rolldown_std_utils::absolute_path_to_relative_slash;

pub fn stabilize_id(module_id: &str, cwd: &Path) -> String {
  let path = Path::new(module_id);
  if path.is_absolute() {
    absolute_path_to_relative_slash(path, cwd)
  } else if module_id.starts_with('\0') {
    // handle virtual modules
    module_id.replace('\0', "\\0")
  } else {
    module_id.to_string()
  }
}

#[test]
fn test_stabilize_id() {
  use rolldown_std_utils::PathExt as _;

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
