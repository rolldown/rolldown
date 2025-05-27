use std::{borrow::Cow, ffi::OsStr, path::Path};

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn representative_file_name(&self) -> Cow<str>;
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
  fn representative_file_name(&self) -> Cow<str> {
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
pub fn representative_file_name_for_preserve_modules(path: &Path) -> (Cow<str>, Cow<str>) {
  let file_name =
    path.file_stem().map_or_else(|| path.to_string_lossy(), |stem| stem.to_string_lossy());
  let idx = path.to_string_lossy().rfind(file_name.as_ref()).expect("should contains file_name");
  let ab_path = slice_cow_str(path.to_string_lossy(), 0, idx + file_name.len());
  (file_name, ab_path)
}

#[inline]
fn slice_cow_str(cow: Cow<str>, start: usize, end: usize) -> Cow<'_, str> {
  match cow {
    Cow::Borrowed(s) => Cow::Borrowed(&s[start..end]),
    Cow::Owned(s) => Cow::Owned(s[start..end].to_string()),
  }
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

  #[cfg(not(target_os = "windows"))]
  {
    let path = cwd.join("src").join("vue.js");
    assert_eq!(representative_file_name_for_preserve_modules(&path).1, "./project/src/vue");
  }
}
