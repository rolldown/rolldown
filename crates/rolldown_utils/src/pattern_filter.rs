use memchr::memmem;
use std::{
  borrow::Cow,
  path::{Path, PathBuf},
};

use crate::js_regex::HybridRegex;
use fast_glob::glob_match;
use rolldown_std_utils::normalize_path_buf_to_slash;

#[derive(Debug, Clone)]
pub enum StringOrRegex {
  String(String),
  Regex(HybridRegex),
}

impl StringOrRegex {
  pub fn try_into_string(self) -> anyhow::Result<String> {
    match self {
      StringOrRegex::String(s) => Ok(s),
      StringOrRegex::Regex(_) => anyhow::bail!("expected a string, but got a regex"),
    }
  }

  pub fn try_into_regex(self) -> anyhow::Result<HybridRegex> {
    match self {
      StringOrRegex::Regex(s) => Ok(s),
      StringOrRegex::String(_) => anyhow::bail!("expected a regex, but got a string"),
    }
  }
}

impl AsRef<StringOrRegex> for StringOrRegex {
  fn as_ref(&self) -> &StringOrRegex {
    self
  }
}
pub enum StringOrRegexMatchKind<'a> {
  Code,
  // The cwd of id
  Id(&'a str),
}

impl StringOrRegex {
  pub fn new(value: String, flag: Option<&String>) -> anyhow::Result<Self> {
    if let Some(flag) = flag {
      let regex = HybridRegex::with_flags(&value, flag)?;
      Ok(Self::Regex(regex))
    } else {
      Ok(Self::String(value))
    }
  }

  pub fn test(&self, value: &str, match_kind: &StringOrRegexMatchKind) -> bool {
    match self {
      StringOrRegex::String(string_pat) => match match_kind {
        StringOrRegexMatchKind::Code => {
          memmem::find(value.as_bytes(), string_pat.as_bytes()).is_some()
        }
        StringOrRegexMatchKind::Id(cwd) => {
          let glob = get_matcher_string(string_pat, cwd);
          glob_match(glob.as_bytes(), value.as_bytes())
        }
      },
      StringOrRegex::Regex(re) => re.matches(value),
    }
  }
}

/// `id` is the raw path of file used for `regex` testing
/// Using `FilterResult` rather than `bool` for complicated scenario, e.g.
/// If you have only one filter, just use `FilterResult#inner` to determine if the `id` is matched,
/// for multiple filters, you should use `FilterResult` to determine if the `id` is matched.
/// See doc of [FilterResult]
pub fn filter(
  exclude: Option<&[StringOrRegex]>,
  include: Option<&[StringOrRegex]>,
  id: &str,
  cwd: &str,
) -> FilterResult {
  let normalized_id = normalize_path(id);
  if let Some(exclude) = exclude {
    for pattern in exclude {
      let v = pattern.as_ref().test(&normalized_id, &StringOrRegexMatchKind::Id(cwd));
      if v {
        return FilterResult::Match(false);
      }
    }
  }
  if let Some(include) = include {
    for pattern in include {
      let v = pattern.as_ref().test(&normalized_id, &StringOrRegexMatchKind::Id(cwd));
      if v {
        return FilterResult::Match(true);
      }
    }
  }
  // If the path is neither matched the exclude nor include,
  // it should only considered should be included if the include pattern is empty
  match include {
    None => FilterResult::NoneMatch(true),
    Some(include) => FilterResult::NoneMatch(include.is_empty()),
  }
}

/// Whether resolving `glob` into a matcher depends on `cwd`. Relative globs are
/// joined onto `cwd`; `**`-prefixed and absolute globs are used as-is (so their
/// resolution is independent of `cwd`).
pub(crate) fn glob_matcher_depends_on_cwd(glob: &str) -> bool {
  !(glob.starts_with("**") || Path::new(glob).is_absolute())
}

/// https://github.com/rollup/plugins/blob/e1a5ef99f1578eb38a8c87563cb9651db228f3bd/packages/pluginutils/src/createFilter.ts#L10
pub fn get_matcher_string<'a>(glob: &'a str, cwd: &'a str) -> Cow<'a, str> {
  if !glob_matcher_depends_on_cwd(glob) {
    return normalize_path(glob);
  }
  // Like Rollup, resolve a relative `cwd` against the process cwd.
  let resolved =
    if Path::new(cwd).has_root() { Ok(PathBuf::from(cwd)) } else { std::path::absolute(cwd) };
  let Ok(cwd) = resolved else {
    // An empty `cwd` can't be resolved; keep the glob relative.
    return Cow::Owned(normalize_path_buf_to_slash(Path::new(cwd).join(glob)));
  };
  // `cwd` is a literal path, so escape its glob syntax, then join `glob` onto it as
  // `/`-separated text. The join can't go through `Path`: `\` is the escape
  // character, and `Path` treats it as a separator on Windows.
  let is_glob_syntax = |c: char| matches!(c, '\\' | '*' | '?' | '[' | ']' | '{' | '}' | '!');
  let mut matcher = normalize_path_buf_to_slash(cwd);
  if matcher.contains(is_glob_syntax) {
    let mut escaped = String::with_capacity(matcher.len() + 8);
    for c in matcher.chars() {
      if is_glob_syntax(c) {
        escaped.push('\\');
      }
      escaped.push(c);
    }
    matcher = escaped;
  }
  matcher.truncate(matcher.trim_end_matches('/').len());
  matcher.reserve(glob.len() + 1);
  for segment in normalize_path(glob).split('/') {
    match segment {
      "" | "." => {}
      // At the root (empty on Unix, `C:` on Windows) no `/` is left, so `..` stops there.
      ".." => {
        if let Some(i) = matcher.rfind('/') {
          matcher.truncate(i);
        }
      }
      _ => {
        matcher.push('/');
        matcher.push_str(segment);
      }
    }
  }
  // `glob` resolved to the root.
  if !matcher.contains('/') {
    matcher.push('/');
  }
  Cow::Owned(matcher)
}

pub fn normalize_path(path: &str) -> Cow<'_, str> {
  #[cfg(windows)]
  {
    use cow_utils::CowUtils;
    path.cow_replace('\\', "/")
  }
  #[cfg(not(windows))]
  {
    Cow::Borrowed(path)
  }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FilterResult {
  /// `Match(true)` means it is matched by `included`,
  /// `Match(false)` means it is matched by `excluded`
  Match(bool),
  /// `NoneMatch(true)` means it is neither matched by `excluded` nor `included`, and the `include` is empty
  /// `NoneMatch(false)` means it is neither matched by `excluded` nor `included`, and the `include` is not empty
  /// You should determine according to the context.
  NoneMatch(bool),
}

impl FilterResult {
  pub fn inner(&self) -> bool {
    match self {
      FilterResult::Match(v) | FilterResult::NoneMatch(v) => *v,
    }
  }
}

/// Same as above but for `code`
pub fn filter_code(
  exclude: Option<&[impl AsRef<StringOrRegex>]>,
  include: Option<&[impl AsRef<StringOrRegex>]>,
  code: &str,
) -> FilterResult {
  if let Some(exclude) = exclude {
    for pattern in exclude {
      let v = pattern.as_ref().test(code, &StringOrRegexMatchKind::Code);
      if v {
        return FilterResult::Match(false);
      }
    }
  }
  if let Some(include) = include {
    for pattern in include {
      let v = match pattern.as_ref() {
        StringOrRegex::String(pattern) => {
          memmem::find(code.as_bytes(), pattern.as_bytes()).is_some()
        }
        StringOrRegex::Regex(re) => re.matches(code),
      };
      if v {
        return FilterResult::Match(true);
      }
    }
  }
  // If the code is neither matched the exclude nor include,
  // it should only considered should be included if the include pattern is empty
  match include {
    None => FilterResult::NoneMatch(true),
    Some(include) => FilterResult::NoneMatch(include.is_empty()),
  }
}

#[cfg(test)]
mod tests {
  use std::path;

  use super::*;

  #[test]
  fn test_filter() {
    #[derive(Debug)]
    struct InputFilter {
      exclude: Option<[StringOrRegex; 1]>,
      include: Option<[StringOrRegex; 1]>,
    }
    /// id, expected
    type TestCase<'a> = (&'a str, FilterResult);
    struct TestCases<'a> {
      input_filter: InputFilter,
      cases: Vec<TestCase<'a>>,
    }

    #[expect(clippy::unnecessary_wraps)]
    fn glob_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), None).unwrap()])
    }
    #[expect(clippy::unnecessary_wraps)]
    fn regex_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), Some(&String::new())).unwrap()])
    }

    let foo_js = "foo.js";
    let resolved_foo_js = path::absolute(foo_js).unwrap().to_string_lossy().into_owned();
    let full_virtual_path = "\0".to_string() + &resolved_foo_js;

    let cases = [
      TestCases {
        input_filter: InputFilter { exclude: None, include: glob_filter("foo.js") },
        cases: vec![
          ("foo.js", FilterResult::Match(true)),
          ("foo.ts", FilterResult::NoneMatch(false)),
          (foo_js, FilterResult::Match(true)),
          ("\0foo.js", FilterResult::NoneMatch(false)),
          (&full_virtual_path, FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: None, include: glob_filter("*.js") },
        cases: vec![
          ("foo.js", FilterResult::Match(true)),
          ("foo.ts", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: None, include: regex_filter("\\.js$") },
        cases: vec![
          ("foo.js", FilterResult::Match(true)),
          ("foo.ts", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: None, include: regex_filter("/foo\\.js$") },
        cases: vec![
          ("a/foo.js", FilterResult::Match(true)),
          #[cfg(windows)]
          ("a\\foo.js", FilterResult::Match(true)),
          ("a_foo.js", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: glob_filter("foo.js"), include: None },
        cases: vec![
          ("foo.js", FilterResult::Match(false)),
          ("foo.ts", FilterResult::NoneMatch(true)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: glob_filter("*.js"), include: None },
        cases: vec![
          ("foo.js", FilterResult::Match(false)),
          ("foo.ts", FilterResult::NoneMatch(true)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: regex_filter("\\.js$"), include: None },
        cases: vec![
          ("foo.js", FilterResult::Match(false)),
          ("foo.ts", FilterResult::NoneMatch(true)),
        ],
      },
      TestCases {
        input_filter: InputFilter {
          exclude: glob_filter("bar.js"),
          include: glob_filter("foo.js"),
        },
        cases: vec![
          ("foo.js", FilterResult::Match(true)),
          ("bar.js", FilterResult::Match(false)),
          ("baz.js", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: glob_filter("foo.*"), include: glob_filter("*.js") },
        cases: vec![
          ("foo.js", FilterResult::Match(false)), // exclude has higher priority
          ("bar.js", FilterResult::Match(true)),
          ("foo.ts", FilterResult::Match(false)),
        ],
      },
      // https://github.com/rolldown/rolldown/issues/3970
      TestCases {
        input_filter: InputFilter { include: glob_filter("/virtual/foo"), exclude: None },
        cases: vec![
          ("/virtual/foo", FilterResult::Match(true)), // exclude has higher priority
        ],
      },
    ];

    for (i, test_case) in cases.into_iter().enumerate() {
      for (si, (id, expected)) in test_case.cases.into_iter().enumerate() {
        let result = filter(
          test_case.input_filter.exclude.as_ref().map(|v| &v[..]),
          test_case.input_filter.include.as_ref().map(|v| &v[..]),
          id,
          "",
        );
        assert_eq!(
          result, expected,
          r"Failed at case {i},
subcase: {si},
filter: {:?}, id: {id}",
          test_case.input_filter
        );
      }
    }
  }

  #[test]
  fn relative_current_dir_globs_match_like_clean_globs() {
    #[cfg(windows)]
    let (cwd, matching_id, nonmatching_id) =
      (r"C:\home\rolldown", r"C:\home\rolldown\foo\file.txt", r"C:\home\rolldown\bar\file.txt");
    #[cfg(not(windows))]
    let (cwd, matching_id, nonmatching_id) =
      ("/home/rolldown", "/home/rolldown/foo/file.txt", "/home/rolldown/bar/file.txt");

    let dirty = get_matcher_string("./foo/*.txt", cwd);
    let clean = get_matcher_string("foo/*.txt", cwd);
    assert_eq!(dirty, clean);

    for glob in ["./foo/*.txt", "foo/*.txt"] {
      let pattern = [StringOrRegex::new(glob.to_string(), None).unwrap()];
      assert_eq!(filter(None, Some(&pattern), matching_id, cwd), FilterResult::Match(true));
      assert_eq!(filter(None, Some(&pattern), nonmatching_id, cwd), FilterResult::NoneMatch(false),);
      assert_eq!(filter(Some(&pattern), None, matching_id, cwd), FilterResult::Match(false));
      assert_eq!(filter(Some(&pattern), None, nonmatching_id, cwd), FilterResult::NoneMatch(true),);
    }
  }

  #[test]
  fn relative_globs_treat_cwd_as_a_literal_path() {
    #[cfg(windows)]
    let cwds = [r"C:\w\my[app]", r"C:\w\{client}", r"C:\w\a!b"];
    #[cfg(not(windows))]
    let cwds = ["/w/my[app]", "/w/{client}", "/w/a!b", "/w/a*b", "/w/q?", r"/w/back\slash"];

    for cwd in cwds {
      let src = format!("{cwd}/src/a.js");
      let other = format!("{cwd}/other/a.js");
      let include = [StringOrRegex::new("src/**".to_string(), None).unwrap()];
      assert_eq!(filter(None, Some(&include), &src, cwd), FilterResult::Match(true), "{cwd}");
      assert_eq!(
        filter(None, Some(&include), &other, cwd),
        FilterResult::NoneMatch(false),
        "{cwd}"
      );
      assert_eq!(filter(Some(&include), None, &src, cwd), FilterResult::Match(false), "{cwd}");

      // `..` may leave `cwd`; the remaining ancestors are still literal.
      let sibling = [StringOrRegex::new("../shared/**".to_string(), None).unwrap()];
      let shared = format!("{cwd}/shared/a.js");
      assert_eq!(
        filter(None, Some(&sibling), &shared, &format!("{cwd}/web")),
        FilterResult::Match(true)
      );
    }
  }

  #[cfg(not(windows))]
  #[test]
  fn relative_globs_keep_their_own_glob_syntax() {
    let include = [StringOrRegex::new("../my[app]/**".to_string(), None).unwrap()];
    assert_eq!(
      filter(None, Some(&include), "/w/mya/a.js", "/w/my[app]"),
      FilterResult::Match(true)
    );
    let include = [StringOrRegex::new("../../src/**".to_string(), None).unwrap()];
    assert_eq!(filter(None, Some(&include), "/src/a.js", "/w"), FilterResult::Match(true));
  }

  #[test]
  fn relative_globs_resolve_a_relative_cwd_against_the_process_cwd() {
    let id = std::env::current_dir().unwrap().join("proj").join("src").join("a.js");
    let include = [StringOrRegex::new("src/**".to_string(), None).unwrap()];
    assert_eq!(
      filter(None, Some(&include), &id.to_string_lossy(), "proj"),
      FilterResult::Match(true)
    );
  }

  #[test]
  fn relative_glob_matcher_strings() {
    // `C:` makes the paths absolute on Windows.
    let root = if cfg!(windows) { "C:" } else { "" };
    let mut cases = vec![
      ("src/**", "/w/my[app]", r"/w/my\[app\]/src/**"),
      ("src/**", "/w/{c},!/", r"/w/\{c\},\!/src/**"),
      // `..` leaves `cwd`, and the glob keeps its own syntax.
      ("../x[y]/**", "/w/my[app]/web", r"/w/my\[app\]/x[y]/**"),
      ("../../../x", "/w", "/x"),
      ("..", "/w", "/"),
    ];
    // Windows forbids `*` and `?` in names, and reads `\` as a separator.
    if cfg!(not(windows)) {
      cases.push(("src/**", r"/w/a*b?\c", r"/w/a\*b\?\\c/src/**"));
    }
    for (glob, cwd, expected) in cases {
      assert_eq!(get_matcher_string(glob, &format!("{root}{cwd}")), format!("{root}{expected}"));
    }
    // An empty `cwd` keeps the glob relative, with `.` and `..` resolved.
    assert_eq!(get_matcher_string("./foo/*.txt", ""), "foo/*.txt");
    assert_eq!(get_matcher_string("src/../foo/*.txt", ""), "foo/*.txt");

    // Before the fix, `[app]` was a character class, so `src/**` also matched in `/w/mya`.
    let cwd = format!("{root}/w/my[app]");
    let include = [StringOrRegex::new("src/**".to_string(), None).unwrap()];
    let matches = |id: &str| filter(None, Some(&include), &format!("{root}{id}"), &cwd);
    assert_eq!(matches("/w/my[app]/src/a.js"), FilterResult::Match(true));
    assert_eq!(matches("/w/mya/src/a.js"), FilterResult::NoneMatch(false));
  }

  #[test]
  fn test_code_filter() {
    #[derive(Debug)]
    struct InputFilter {
      exclude: Option<[StringOrRegex; 1]>,
      include: Option<[StringOrRegex; 1]>,
    }
    /// code, expected
    type TestCase<'a> = (&'a str, FilterResult);
    struct TestCases<'a> {
      input_filter: InputFilter,
      cases: Vec<TestCase<'a>>,
    }

    #[expect(clippy::unnecessary_wraps)]
    fn string_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), None).unwrap()])
    }
    #[expect(clippy::unnecessary_wraps)]
    fn regex_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), Some(&String::new())).unwrap()])
    }

    let cases = [
      TestCases {
        input_filter: InputFilter { exclude: None, include: string_filter("import.meta") },
        cases: vec![
          ("import.meta", FilterResult::Match(true)),
          ("import_meta", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: None, include: regex_filter("import\\.\\w+") },
        cases: vec![
          ("import.meta", FilterResult::Match(true)),
          ("import_meta", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: string_filter("import.meta"), include: None },
        cases: vec![
          ("import.meta", FilterResult::Match(false)),
          ("import_meta", FilterResult::NoneMatch(true)),
        ],
      },
      // Test none ascii
      TestCases {
        input_filter: InputFilter { include: string_filter("你好"), exclude: None },
        cases: vec![
          ("世界你好 hello world", FilterResult::Match(true)),
          ("import_meta", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter { exclude: regex_filter("import\\.\\w+"), include: None },
        cases: vec![
          ("import.meta", FilterResult::Match(false)),
          ("import_meta", FilterResult::NoneMatch(true)),
        ],
      },
      TestCases {
        input_filter: InputFilter {
          exclude: string_filter("import_meta"),
          include: string_filter("import.meta"),
        },
        cases: vec![
          ("import.meta", FilterResult::Match(true)),
          ("import_meta", FilterResult::Match(false)),
          ("importmeta", FilterResult::NoneMatch(false)),
        ],
      },
      TestCases {
        input_filter: InputFilter {
          exclude: regex_filter("\\w+\\.meta"),
          include: regex_filter("import\\.\\w+"),
        },
        cases: vec![
          ("import.meta", FilterResult::Match(false)), // exclude has higher priority
          ("import.foo", FilterResult::Match(true)),
          ("foo.meta", FilterResult::Match(false)),
        ],
      },
    ];

    for test_case in cases {
      for (code, expected) in test_case.cases {
        let result = filter_code(
          test_case.input_filter.exclude.as_ref().map(|v| &v[..]),
          test_case.input_filter.include.as_ref().map(|v| &v[..]),
          code,
        );
        assert_eq!(result, expected, "filter: {:?}, code: {code}", test_case.input_filter);
      }
    }
  }
}
