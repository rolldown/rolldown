/// According to the doc of `regress`, https://docs.rs/regress/0.10.0/regress/#comparison-to-regex-crate
/// **regress supports features that regex does not, in particular backreferences and zero-width lookaround assertions.**
/// these features are not commonly used, so in most cases the slow path will not be reached.
#[derive(Debug)]
pub enum HybridRegex {
  Optimize(regex::Regex),
  Ecma(regress::Regex),
}

impl HybridRegex {
  pub fn new(source: &str) -> anyhow::Result<Self> {
    match regex::Regex::new(source).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::new(source).map(HybridRegex::Ecma).map_err(anyhow::Error::from),
    }
  }

  pub fn matches(&self, text: &str) -> bool {
    match self {
      HybridRegex::Optimize(reg) => reg.is_match(text),
      HybridRegex::Ecma(reg) => reg.find(text).is_some(),
    }
  }
}
