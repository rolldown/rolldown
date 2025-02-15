use rolldown_utils::pattern_filter::StringOrRegex;

use crate::utils::normalize_path;

#[derive(Debug)]
pub struct UtilsFilter {
  include: UtilsSingleFilter,
  exclude: UtilsSingleFilter,
  has_include_matcher: bool,
}

impl UtilsFilter {
  /// works like `createFilter` from `@rollup/pluginutils`
  /// only supports `options.resolve: false`
  pub fn new(include: Vec<StringOrRegex>, exclude: Vec<StringOrRegex>) -> Self {
    Self {
      has_include_matcher: !include.is_empty(),
      include: UtilsSingleFilter::new(include),
      exclude: UtilsSingleFilter::new(exclude),
    }
  }

  pub fn is_match(&self, id: &str) -> bool {
    if id.contains('\0') {
      return false;
    }

    let normalized = normalize_path(id);
    if self.exclude.is_match(&normalized) {
      return false;
    }
    if self.include.is_match(&normalized) {
      return true;
    }
    !self.has_include_matcher
  }
}

#[derive(Debug)]
struct UtilsSingleFilter(Vec<StringOrRegex>);

impl UtilsSingleFilter {
  fn new(filter: Vec<StringOrRegex>) -> Self {
    Self(filter.into_iter().map(normalize_string_or_regex).collect())
  }

  fn is_match(&self, input: &str) -> bool {
    self.0.iter().any(|filter| match filter {
      StringOrRegex::String(s) => fast_glob::glob_match(s, input),
      StringOrRegex::Regex(regex) => regex.matches(input),
    })
  }
}

fn normalize_string_or_regex(value: StringOrRegex) -> StringOrRegex {
  match value {
    StringOrRegex::String(s) => StringOrRegex::String(normalize_path(&s).into_owned()),
    StringOrRegex::Regex(regex) => StringOrRegex::Regex(regex),
  }
}
