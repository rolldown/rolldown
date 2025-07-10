use rolldown_utils::pattern_filter::StringOrRegex;

use crate::utils_filter::UtilsFilter;

#[derive(Debug, Clone)]
pub enum ResolveOptionsExternal {
  True,
  Vec(Vec<String>),
}

impl ResolveOptionsExternal {
  pub fn is_external_explicitly(&self, id: &str) -> bool {
    let vec = match self {
      ResolveOptionsExternal::Vec(vec) => vec,
      _ => return false,
    };
    vec.iter().any(|v| v == id)
  }
}

#[derive(Debug)]
pub struct ResolveOptionsNoExternal(ResolveOptionsNoExternalInner);

impl ResolveOptionsNoExternal {
  pub fn new_true() -> Self {
    Self(ResolveOptionsNoExternalInner::True)
  }

  pub fn new_vec(value: Vec<StringOrRegex>) -> Self {
    if value.is_empty() {
      Self(ResolveOptionsNoExternalInner::Empty)
    } else {
      Self(ResolveOptionsNoExternalInner::Vec(UtilsFilter::new(vec![], value)))
    }
  }

  pub fn is_true(&self) -> bool {
    matches!(self.0, ResolveOptionsNoExternalInner::True)
  }

  pub fn is_no_external(&self, id: &str) -> bool {
    match &self.0 {
      ResolveOptionsNoExternalInner::True => true,
      ResolveOptionsNoExternalInner::Vec(filter) => !filter.is_match(id),
      ResolveOptionsNoExternalInner::Empty => false,
    }
  }
}

#[derive(Debug)]
enum ResolveOptionsNoExternalInner {
  True,
  Vec(UtilsFilter),
  Empty,
}
