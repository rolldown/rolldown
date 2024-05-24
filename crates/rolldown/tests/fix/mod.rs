#[cfg(test)]
mod fix_package_json_test_case {
  use crate::common::Case;
  use rolldown::Bundler;
  use rolldown_common::BundlerOptions;
  use std::env;
  use std::ffi::OsString;
  use std::path::Path;

  pub fn path_resolve(path: &str) -> String {
    let work_cwd = {
      match env::var("CARGO_MANIFEST_DIR") {
        Ok(_val) => env!("CARGO_MANIFEST_DIR").to_string(),
        Err(_) => match std::env::current_exe() {
          Ok(val) => val.parent().unwrap().to_str().unwrap().to_string(),
          Err(_) => std::env::current_dir().unwrap().to_str().unwrap().to_string(),
        },
      }
    };
    let os_work_cwd = OsString::from(work_cwd);
    Path::new(&os_work_cwd).join(path).into_os_string().into_string().unwrap()
  }

  #[test]
  fn test_package_json_case() {
    let dir =
      path_resolve(r"tests/esbuild/packagejson/test_package_json_syntax_error_trailing_comma");
    let config_path = Path::new(dir.as_str());
    Case::new(config_path).run();
  }
}
