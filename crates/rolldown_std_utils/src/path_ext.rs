use cow_utils;
use std::{borrow::Cow, ffi::OsStr, path::Path};

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn representative_file_name(&self, absolute: bool) -> Cow<str>;
}

impl PathExt for std::path::Path {
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
  fn representative_file_name(&self, absolute: bool) -> Cow<str> {
    let file_name =
      self.file_stem().map_or_else(|| self.to_string_lossy(), |stem| stem.to_string_lossy());

    let file_name = match &*file_name {
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
    };

    if absolute {
      let idx =
        self.to_string_lossy().rfind(file_name.as_ref()).expect("should contains file_name");
      Cow::Owned(self.to_string_lossy()[0..idx + file_name.len()].to_string())
    } else {
      file_name
    }
  }
}

#[test]
fn test_representative_file_name() {
  let cwd = Path::new(".").join("project");
  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(false), "vue");

  let path = cwd.join("vue").join("index.js");
  assert_eq!(path.representative_file_name(false), "vue");

  let path = cwd.join("vue").join("mod.ts");
  assert_eq!(path.representative_file_name(false), "vue");

  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(true), "./project/src/vue");
}
