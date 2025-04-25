// @cSpell:ignore subcase
use memchr::memmem;
use std::{borrow::Cow, path::Path};

use crate::js_regex::HybridRegex;
use fast_glob::glob_match;

#[derive(Debug, Clone)]
pub enum StringOrRegex {
  String(String),
  Regex(HybridRegex),
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
  pub fn new(value: String, flag: &Option<String>) -> anyhow::Result<Self> {
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
  exclude: Option<&[impl AsRef<StringOrRegex>]>,
  include: Option<&[impl AsRef<StringOrRegex>]>,
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

/// https://github.com/rollup/plugins/blob/e1a5ef99f1578eb38a8c87563cb9651db228f3bd/packages/pluginutils/src/createFilter.ts#L10
fn get_matcher_string<'a>(glob: &'a str, cwd: &'a str) -> Cow<'a, str> {
  if glob.starts_with("**") || Path::new(glob).is_absolute() {
    normalize_path(glob)
  } else {
    let final_path = Path::new(cwd).join(glob);
    Cow::Owned(normalize_path(&final_path.to_string_lossy()).into_owned())
  }
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

#[derive(Debug, PartialEq)]
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

  #[expect(clippy::too_many_lines)]
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
      Some([StringOrRegex::new(value.to_string(), &None).unwrap()])
    }
    #[expect(clippy::unnecessary_wraps)]
    fn regex_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), &Some(String::new())).unwrap()])
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
      Some([StringOrRegex::new(value.to_string(), &None).unwrap()])
    }
    #[expect(clippy::unnecessary_wraps)]
    fn regex_filter(value: &str) -> Option<[StringOrRegex; 1]> {
      Some([StringOrRegex::new(value.to_string(), &Some(String::new())).unwrap()])
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
          // cspell:ignore importmeta
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
