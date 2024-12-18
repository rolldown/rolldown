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
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self, Jsx::Disable)
  }

  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self, Jsx::Preserve)
  }
}
