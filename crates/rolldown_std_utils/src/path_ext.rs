use std::{
  borrow::Cow,
  ffi::OsStr,
  path::{Path, PathBuf},
};

use sugar_path::SugarPath;

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn representative_file_name(&self) -> Cow<'_, str>;
}

impl PathExt for Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }

  fn expect_to_slash(&self) -> String {
    let path = if std::path::MAIN_SEPARATOR == '/' {
      self.to_str().map(Cow::Borrowed)
    } else {
      self.to_str().map(|s| Cow::Owned(s.replace(std::path::MAIN_SEPARATOR, "/")))
    };

    path
      .unwrap_or_else(|| panic!("Failed to convert {:?} to slash str", self.display()))
      .into_owned()
  }

  /// It doesn't ensure the file name is a valid identifier in JS.
  fn representative_file_name(&self) -> Cow<'_, str> {
    let file_name =
      self.file_stem().map_or_else(|| self.to_string_lossy(), |stem| stem.to_string_lossy());

    match &*file_name {
      // "index": Node.js use `index` as a special name for directory import.
      // "mod": https://docs.deno.com/runtime/manual/references/contributing/style_guide#do-not-use-the-filename-indextsindexjs.
      "index" | "mod" => {
        if let Some(parent_dir_name) =
          self.parent().and_then(Path::file_stem).map(OsStr::to_string_lossy)
        {
          parent_dir_name
        } else {
          file_name
        }
      }
      _ => file_name,
    }
  }
}

/// The first one is for chunk name, the second element is used for generate absolute file name
///
/// When `preserve_modules_root` is provided, the chunk name will be relative to that root.
/// Otherwise, it will be relative to `input_base`.
pub fn representative_file_name_for_preserve_modules(
  path: &Path,
  input_base: &str,
  preserve_modules_root: Option<&str>,
) -> (String, String) {
  let ab_path = path.with_extension("").to_string_lossy().into_owned();

  // Compute the chunk name (relative path with directory structure)
  // Try to strip preserve_modules_root first
  let chunk_name = if let Some(root) = preserve_modules_root {
    if ab_path.starts_with(root) {
      ab_path[root.len()..].trim_start_matches(['/', '\\']).to_string()
    } else {
      // Fall back to making it relative to input_base
      PathBuf::from(&ab_path).relative(input_base).to_slash_lossy().into_owned()
    }
  } else {
    // Make it relative to input_base
    PathBuf::from(&ab_path).relative(input_base).to_slash_lossy().into_owned()
  };

  (chunk_name, ab_path)
}

#[test]
fn test_representative_file_name() {
  let cwd = Path::new(".").join("project");
  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("index.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("mod.ts");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("x.jsx");
  let input_base = cwd.to_str().unwrap();
  let (chunk_name, ab_path) = representative_file_name_for_preserve_modules(&path, input_base, None);
  assert_eq!(Path::new(&ab_path).file_name().unwrap().to_string_lossy(), "x");
  assert_eq!(chunk_name, "x");

  #[cfg(not(target_os = "windows"))]
  {
    let path = cwd.join("src").join("vue.js");
    let input_base = cwd.to_str().unwrap();
    assert_eq!(representative_file_name_for_preserve_modules(&path, input_base, None).1, "./project/src/vue");
    assert_eq!(representative_file_name_for_preserve_modules(&path, input_base, None).0, "src/vue");

    // Test with preserve_modules_root
    let preserve_modules_root = cwd.join("src").to_str().map(|s| s.to_string());
    let (chunk_name, _) = representative_file_name_for_preserve_modules(
      &path,
      input_base,
      preserve_modules_root.as_deref(),
    );
    assert_eq!(chunk_name, "vue");
  }
}
