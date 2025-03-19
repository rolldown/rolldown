use oxc::transformer::JsxOptions;

#[derive(Debug, Clone)]
pub enum Jsx {
  // Disable jsx parser, it will give you a syntax error if you use jsx syntax
  Disable,
  // Disable jsx transformer.
  Preserve,
  // Enable jsx transformer.
  Enable(JsxOptions),
}

impl Default for Jsx {
  fn default() -> Self {
    // default mode is `automatic`
    Jsx::Enable(JsxOptions::default())
  }
}

impl Jsx {
  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self, Jsx::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self, Jsx::Preserve)
  }
}

/// After normalization jsx is already merge into the transform options
#[derive(Debug, Clone, Default)]
pub enum NormalizedJsxOptions {
  Disable,
  Preserve,
  #[default]
  Enable,
}

impl NormalizedJsxOptions {
  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self, NormalizedJsxOptions::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self, NormalizedJsxOptions::Preserve)
  }
}
