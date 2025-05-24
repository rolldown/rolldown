use std::ops::{Deref, DerefMut};

use oxc::transformer::{ESTarget, TransformOptions as OxcTransformOptions};

#[derive(Debug, Default, Clone)]
pub enum JsxPreset {
  // Enable jsx transformer.
  #[default]
  Enable,
  // Disable jsx parser, it will give you a syntax error if you use jsx syntax
  Disable,
  // Disable jsx transformer.
  Preserve,
}

#[derive(Debug, Default, Clone)]

pub struct TransformOptions {
  inner: OxcTransformOptions,
  pub es_target: ESTarget,
  pub jsx_preset: JsxPreset,
}

impl TransformOptions {
  #[inline]
  pub fn new(options: OxcTransformOptions, es_target: ESTarget, jsx_preset: JsxPreset) -> Self {
    Self { inner: options, es_target, jsx_preset }
  }

  #[inline]
  pub fn is_jsx_disabled(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Disable)
  }

  #[inline]
  pub fn is_jsx_preserve(&self) -> bool {
    matches!(self.jsx_preset, JsxPreset::Preserve)
  }
}

impl Deref for TransformOptions {
  type Target = OxcTransformOptions;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl DerefMut for TransformOptions {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}
