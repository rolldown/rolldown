mod compiler_assumptions;
mod decorator_options;
mod jsx_options;
mod transform_options;
mod typescript_options;

pub use {
  compiler_assumptions::CompilerAssumptions,
  decorator_options::DecoratorOptions,
  jsx_options::{JsxOptions, ReactRefreshOptions},
  transform_options::TransformOptions,
  typescript_options::{IsolatedDeclarationsOptions, TypeScriptOptions},
};
