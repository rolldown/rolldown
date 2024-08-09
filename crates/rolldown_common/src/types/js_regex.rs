use regress::Flags;

/// According to the doc of `regress`, https://docs.rs/regress/0.10.0/regress/#comparison-to-regex-crate
/// **regress supports features that regex does not, in particular backreferences and zero-width lookaround assertions.**
/// these features are not commonly used, so in most cases the slow path will not be reached.
#[derive(Debug)]
pub enum HybridRegex {
  Optimize(regex::Regex),
  Ecma(regress::Regex),
}

struct HybridRegexFlags;

impl HybridRegexFlags {
  #[inline]
  pub fn to_regex_flags(chars: &str) -> String {
    // Use an 8-bit bit mask to represent flags
    // Each bit in the mask corresponds to a different flag
    // If a flag appears multiple times, its bit will only be set once
    let mut flags = 0u8;
    for c in chars.bytes() {
      match c {
        b'i' => flags |= 1 << 0, // 0000_0001
        b'm' => flags |= 1 << 1, // 0000_0010
        b's' => flags |= 1 << 2, // 0000_0100
        b'u' => flags |= 1 << 3, // 0000_1000
        b'R' => flags |= 1 << 4, // 0001_0000
        b'U' => flags |= 1 << 5, // 0010_0000
        b'x' => flags |= 1 << 6, // 0100_0000
        _ => {}
      }
    }

    let mut result = String::new();
    if flags & (1 << 0) != 0 {
      result.push('i');
    }
    if flags & (1 << 1) != 0 {
      result.push('m');
    }
    if flags & (1 << 2) != 0 {
      result.push('s');
    }
    if flags & (1 << 3) != 0 {
      result.push('u');
    }
    if flags & (1 << 4) != 0 {
      result.push('R');
    }
    if flags & (1 << 5) != 0 {
      result.push('U');
    }
    if flags & (1 << 6) != 0 {
      result.push('x');
    }

    if result.is_empty() {
      String::default()
    } else {
      format!("(?{result})")
    }
  }

  #[inline]
  pub fn to_regress_flags(chars: &str) -> Flags {
    Flags::from(chars)
  }
}

impl HybridRegex {
  pub fn new(source: &str) -> anyhow::Result<Self> {
    match regex::Regex::new(source).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::new(source).map(HybridRegex::Ecma).map_err(anyhow::Error::from),
    }
  }

  pub fn with_flags(source: &str, flags: &str) -> anyhow::Result<Self> {
    let regex_flags = HybridRegexFlags::to_regex_flags(flags);

    let pattern = if regex_flags.is_empty() { source } else { &format!("{regex_flags}{source}") };

    match regex::Regex::new(pattern).map(HybridRegex::Optimize) {
      Ok(reg) => Ok(reg),
      Err(_) => regress::Regex::with_flags(source, HybridRegexFlags::to_regress_flags(flags))
        .map(HybridRegex::Ecma)
        .map_err(anyhow::Error::from),
    }
  }

  pub fn matches(&self, text: &str) -> bool {
    match self {
      HybridRegex::Optimize(reg) => reg.is_match(text),
      HybridRegex::Ecma(reg) => reg.find(text).is_some(),
    }
  }
}
