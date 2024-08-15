use crate::js_regex::HybridRegex;
use glob_match::glob_match;

#[derive(Debug)]
pub enum StringOrRegex {
  String(String),
  Regex(HybridRegex),
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
}

/// `id` is the raw path of file used for `regex` testing
/// `stable_id` is the relative path for cwd , used for `glob` testing
pub fn filter(
  exclude: &[StringOrRegex],
  include: &[StringOrRegex],
  id: &str,
  stable_id: &str,
) -> bool {
  for pattern in exclude {
    let v = match pattern {
      StringOrRegex::String(glob) => glob_match(glob.as_str(), stable_id),
      StringOrRegex::Regex(re) => re.matches(id),
    };
    if v {
      return false;
    }
  }
  for pattern in include {
    let v = match pattern {
      StringOrRegex::String(glob) => glob_match(glob.as_str(), stable_id),
      StringOrRegex::Regex(re) => re.matches(id),
    };
    if v {
      return true;
    }
  }
  include.is_empty()
}
