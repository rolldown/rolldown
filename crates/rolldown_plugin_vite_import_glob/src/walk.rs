use std::path::PathBuf;

use sugar_path::SugarPath as _;

#[derive(Debug)]
pub struct PathWithGlob<'a> {
  pub path: String,
  pub glob: &'a str,
}

impl<'a> PathWithGlob<'a> {
  pub fn new(mut path: String, glob: &'a str) -> Self {
    let j = Self::split_path_and_glob(&path, glob);
    let i = Self::find_glob_syntax(&glob[glob.len() - j..]);
    path.truncate(path.len() - i);
    Self { path, glob: &glob[glob.len() - i..] }
  }

  fn find_glob_syntax(path: &str) -> usize {
    let mut last_slash = 0;
    for (i, b) in path.as_bytes().iter().enumerate() {
      if *b == b'/' {
        last_slash = i;
      } else if [b'\\', b'*', b'?', b'[', b']', b'{', b'}'].contains(b) {
        return path.len() - last_slash;
      }
    }
    path.len() - last_slash
  }

  fn split_path_and_glob(path: &str, glob: &str) -> usize {
    let path = path.as_bytes();
    let glob = glob.as_bytes();

    let mut num_equal = 0;
    let max_equal = path.len().min(glob.len());
    while num_equal < max_equal {
      let p = path[path.len() - 1 - num_equal];
      let g = glob[glob.len() - 1 - num_equal];
      if p != g {
        break;
      }
      num_equal += 1;
    }

    num_equal
  }
}

pub fn walk_glob_matches<'a>(
  base: &str,
  positive: &'a [PathWithGlob<'a>],
  negative: &'a [PathWithGlob<'a>],
  exhaustive: bool,
  skip_id: &'a str,
) -> impl Iterator<Item = PathBuf> + 'a {
  walkdir::WalkDir::new(base)
    .follow_links(true)
    .sort_by(|a, b| a.file_name().cmp(b.file_name()))
    .into_iter()
    .filter_entry(move |entry| {
      exhaustive || entry.depth() == 0 || {
        let name = entry.file_name();
        if name.as_encoded_bytes().first() == Some(&b'.') {
          return false;
        }
        name.to_str().is_none_or(|s| s != "node_modules")
      }
    })
    .filter_map(Result::ok)
    .filter(|e| !e.file_type().is_dir())
    .filter_map(move |entry| {
      let path = entry.path().to_slash_lossy();
      if skip_id == path {
        return None;
      }
      let matches_rule = |v: &PathWithGlob| -> bool {
        path.strip_prefix(&v.path).map(|p| fast_glob::glob_match(v.glob, p)).unwrap_or(false)
      };
      if negative.iter().any(matches_rule) || !positive.iter().any(matches_rule) {
        return None;
      }
      Some(entry.into_path())
    })
}
