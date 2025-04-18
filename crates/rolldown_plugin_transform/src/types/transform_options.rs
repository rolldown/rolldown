use itertools::Either;

use super::{
  decorator_options::DecoratorOptions, jsx_options::JsxOptions,
  typescript_options::TypeScriptOptions,
};

#[derive(Debug, Default, Clone)]
pub struct TransformOptions {
  pub lang: Option<String>,

  pub jsx: Option<Either<String, JsxOptions>>,

  pub decorator: Option<DecoratorOptions>,

  pub typescript: Option<TypeScriptOptions>,
}
