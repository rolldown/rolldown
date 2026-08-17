use std::{borrow::Cow, ffi::OsStr};

use arcstr::ArcStr;
use cow_utils::CowUtils as _;
use dashmap::Entry;
use sugar_path::SugarPath as _;

use crate::{
  concat_string,
  dashmap::FxDashMap,
  hash_placeholder::{HASH_PLACEHOLDER_LEFT_FINDER, find_hash_placeholders},
};

/// Copy from rust std
fn split_file_at_dot(file: &OsStr) -> (&OsStr, Option<&OsStr>) {
  let slice = file.as_encoded_bytes();
  if slice == b".." {
    return (file, None);
  }

  // The unsafety here stems from converting between &OsStr and &[u8]
  // and back. This is safe to do because (1) we only look at ASCII
  // contents of the encoding and (2) new &OsStr values are produced
  // only from ASCII-bounded slices of existing &OsStr values.
  let i = match slice[1..].iter().position(|b| *b == b'.') {
    Some(i) => i + 1,
    None => return (file, None),
  };
  let before = &slice[..i];
  let after = &slice[i + 1..];
  unsafe {
    (OsStr::from_encoded_bytes_unchecked(before), Some(OsStr::from_encoded_bytes_unchecked(after)))
  }
}

/// Lowercase key for case-insensitive filesystems (macOS APFS, Windows NTFS),
/// except inside hash placeholders (`!~{...}~`): their index alphabet is
/// case-sensitive base64, so folding them would make distinct placeholders
/// (e.g. `!~{00d}~` and `!~{00D}~`) falsely collide.
/// When nothing needs folding, reuse the `candidate` Arc directly to avoid allocation.
fn case_insensitive_key(candidate: &ArcStr) -> ArcStr {
  let s = candidate.as_str();
  let mut placeholders = find_hash_placeholders(s, &HASH_PLACEHOLDER_LEFT_FINDER).peekable();
  if placeholders.peek().is_none() {
    return match s.cow_to_ascii_lowercase() {
      Cow::Borrowed(_) => candidate.clone(),
      Cow::Owned(owned) => owned.into(),
    };
  }
  let mut key = String::with_capacity(s.len());
  let mut cursor = 0;
  for (start, end, placeholder) in placeholders {
    key.push_str(&s[cursor..start].cow_to_ascii_lowercase());
    key.push_str(placeholder);
    cursor = end;
  }
  key.push_str(&s[cursor..].cow_to_ascii_lowercase());
  key.into()
}

pub fn make_unique_name(name: &ArcStr, used_name_counts: &FxDashMap<ArcStr, u32>) -> ArcStr {
  let mut candidate = name.clone();
  let extension = name
    .as_path()
    .file_name()
    .map(split_file_at_dot)
    .and_then(|(_before, after)| after)
    .and_then(OsStr::to_str)
    .map(|e| concat_string!(".", e))
    .unwrap_or_default();
  let file_name = &name[..name.len() - extension.len()];
  loop {
    match used_name_counts.entry(case_insensitive_key(&candidate)) {
      Entry::Occupied(mut occ) => {
        // This name is already used
        let next_count = *occ.get();
        occ.insert(next_count + 1);
        candidate = ArcStr::from(concat_string!(
          file_name,
          itoa::Buffer::new().format(next_count),
          extension
        ));
      }
      Entry::Vacant(vac) => {
        vac.insert(2);
        break candidate;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test() {
    let used_name_counts = FxDashMap::default();

    let unique_name = make_unique_name(&ArcStr::from("foo.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo.js");

    let unique_name = make_unique_name(&ArcStr::from("foo.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo2.js");

    let unique_name = make_unique_name(&ArcStr::from("foo2.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo22.js");
  }

  #[test]
  fn test2() {
    let used_name_counts = FxDashMap::default();

    let unique_name = make_unique_name(&ArcStr::from("foo.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo.js");

    let unique_name = make_unique_name(&ArcStr::from("foo2.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo2.js");

    let unique_name = make_unique_name(&ArcStr::from("foo.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo3.js");
  }

  #[test]
  fn double_dot_extension() {
    let used_name_counts = FxDashMap::default();

    let unique_name = make_unique_name(&ArcStr::from("foo.d.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo.d.js");

    let unique_name = make_unique_name(&ArcStr::from("foo.d.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "foo2.d.js");
  }

  #[test]
  fn hash_placeholders_stay_case_sensitive() {
    let used_name_counts = FxDashMap::default();

    // `!~{00d}~` (index 13) and `!~{00D}~` (index 39) are distinct placeholders
    let unique_name = make_unique_name(&ArcStr::from("chunks/!~{00d}~.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "chunks/!~{00d}~.js");

    let unique_name = make_unique_name(&ArcStr::from("chunks/!~{00D}~.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "chunks/!~{00D}~.js");

    // an identical placeholder name still collides
    let unique_name = make_unique_name(&ArcStr::from("chunks/!~{00D}~.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "chunks/!~{00D}~2.js");

    // the rest of the name is still folded case-insensitively
    let unique_name = make_unique_name(&ArcStr::from("CHUNKS/!~{00d}~.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "CHUNKS/!~{00d}~2.js");
  }

  #[test]
  fn case_insensitive() {
    let used_name_counts = FxDashMap::default();

    // "Edit.js" is registered first (keeps original case)
    let unique_name = make_unique_name(&ArcStr::from("Edit.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "Edit.js");

    // "edit.js" conflicts with "Edit.js" on case-insensitive filesystems
    let unique_name = make_unique_name(&ArcStr::from("edit.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "edit2.js");

    // "EDIT.js" also conflicts
    let unique_name = make_unique_name(&ArcStr::from("EDIT.js"), &used_name_counts);
    assert_eq!(unique_name.as_str(), "EDIT3.js");
  }
}
