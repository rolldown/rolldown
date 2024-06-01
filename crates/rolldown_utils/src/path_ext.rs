use std::{borrow::Cow, ffi::OsStr, path::Path};

use sugar_path::SugarPath;

use crate::ecma_script::legitimize_identifier_name;

pub trait PathExt {
  fn expect_to_str(&self) -> &str;

  fn expect_to_slash(&self) -> String;

  fn representative_file_name(&self) -> Cow<str>;
}

impl PathExt for std::path::Path {
  fn expect_to_str(&self) -> &str {
    self.to_str().unwrap_or_else(|| {
      panic!("Failed to convert {:?} to valid utf8 str", self.display());
    })
  }

  fn expect_to_slash(&self) -> String {
    self
      .to_slash()
      .unwrap_or_else(|| panic!("Failed to convert {:?} to slash str", self.display()))
      .into_owned()
  }

  fn representative_file_name(&self) -> Cow<str> {
    let file_name =
      self.file_stem().map_or_else(|| self.to_string_lossy(), |stem| stem.to_string_lossy());

    let file_name = match &*file_name {
      // "index": Node.js use `index` as a special name for directory import.
      // "mod": https://docs.deno.com/runtime/manual/references/contributing/style_guide#do-not-use-the-filename-indextsindexjs.
      "index" | "mod" => {
        if let Some(parent_dir_name) =
          self.parent().and_then(Path::file_stem).map(OsStr::to_string_lossy)
        {
          Cow::Owned([&*parent_dir_name, "_", &*file_name].concat())
        } else {
          file_name
        }
      }
      _ => file_name,
    };

    let legal = legitimize_identifier_name(&file_name);
    match legal {
      // No changes. Just return the original file name.
      Cow::Borrowed(_) => file_name,
      Cow::Owned(v) => Cow::Owned(v),
    }
  }
}

#[test]
fn test_representative_file_name() {
  let cwd = Path::new(".").join("project");
  let path = cwd.join("src").join("vue.js");
  assert_eq!(path.representative_file_name(), "vue");

  let path = cwd.join("vue").join("index.js");
  assert_eq!(path.representative_file_name(), "vue_index");

  let path = cwd.join("vue").join("mod.ts");
  assert_eq!(path.representative_file_name(), "vue_mod");
}
