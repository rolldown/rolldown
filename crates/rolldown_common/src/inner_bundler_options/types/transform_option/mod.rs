mod compiler_assumptions;
mod decorator_options;
mod jsx_options;
mod plugin_options;
mod typescript_options;

use oxc::transformer::{EnvOptions, HelperLoaderOptions};

pub use itertools::Either;
pub use {
  compiler_assumptions::CompilerAssumptions,
  decorator_options::DecoratorOptions,
  jsx_options::{JsxOptions, ReactRefreshOptions},
  plugin_options::{PluginsOptions, StyledComponentsOptions},
  typescript_options::{IsolatedDeclarationsOptions, TypeScriptOptions},
};

#[cfg(debug_assertions)]
use super::transform_options::JsxPreset;

#[derive(Debug, Default, Clone)]
pub struct TransformOptions {
  /// Configure how TSX and JSX are transformed.
  pub jsx: Option<Either<String, JsxOptions>>,

  /// Override the jsx preset derived from `jsx` option. Only used for testing.
  #[cfg(debug_assertions)]
  pub jsx_preset: Option<JsxPreset>,

  /// Sets the target environment for the generated JavaScript.
  ///
  /// The lowest target is `es2015`.
  ///
  /// Example:
  ///
  /// * `'es2015'`
  /// * `['es2020', 'chrome58', 'edge16', 'firefox57', 'node12', 'safari11']`
  ///
  /// @default `esnext` (No transformation)
  ///
  /// @see [esbuild#target](https://esbuild.github.io/api/#target)
  pub target: Option<Either<String, Vec<String>>>,

  /// Set assumptions in order to produce smaller output.
  pub assumptions: Option<CompilerAssumptions>,

  /// Decorator plugin
  pub decorator: Option<DecoratorOptions>,

  /// Configure how TypeScript is transformed.
  pub typescript: Option<TypeScriptOptions>,

  /// Third-party plugins to use.
  pub plugins: Option<PluginsOptions>,

  /// Behaviour for runtime helpers.
  pub helpers: Option<HelperLoaderOptions>,
}

impl From<crate::utils::enhanced_transform::EnhancedTransformOptions> for TransformOptions {
  fn from(options: crate::utils::enhanced_transform::EnhancedTransformOptions) -> Self {
    Self {
      jsx: options.jsx,
      #[cfg(debug_assertions)]
      jsx_preset: None,
      target: options.target,
      assumptions: options.assumptions,
      decorator: options.decorator,
      typescript: options.typescript,
      plugins: options.plugins,
      helpers: options.helpers,
    }
  }
}

impl From<TransformOptions> for crate::utils::enhanced_transform::EnhancedTransformOptions {
  fn from(options: TransformOptions) -> Self {
    Self {
      jsx: options.jsx,
      target: options.target,
      assumptions: options.assumptions,
      decorator: options.decorator,
      typescript: options.typescript,
      plugins: options.plugins,
      helpers: options.helpers,
      // These fields are not present in TransformOptions
      cwd: None,
      source_type: None,
      tsconfig: None,
      sourcemap: false,
      input_map: None,
      define: None,
      inject: None,
    }
  }
}

impl TryFrom<TransformOptions> for oxc::transformer::TransformOptions {
  type Error = String;

  fn try_from(options: TransformOptions) -> Result<Self, Self::Error> {
    let env = match options.target {
      Some(Either::Left(s)) => EnvOptions::from_target(&s)?,
      Some(Either::Right(list)) => EnvOptions::from_target_list(&list)?,
      _ => EnvOptions::default(),
    };
    Ok(Self {
      // cwd: options.cwd.map(PathBuf::from).unwrap_or_default(),
      assumptions: options.assumptions.map(Into::into).unwrap_or_default(),
      typescript: options
        .typescript
        .map(oxc::transformer::TypeScriptOptions::from)
        .unwrap_or_default(),
      decorator: options
        .decorator
        .map(oxc::transformer::DecoratorOptions::from)
        .unwrap_or_default(),
      jsx: match options.jsx {
        Some(Either::Left(s)) => {
          if s == "preserve" {
            oxc::transformer::JsxOptions::disable()
          } else {
            return Err(format!("Invalid jsx option: `{s}`."));
          }
        }
        Some(Either::Right(options)) => oxc::transformer::JsxOptions::from(options),
        None => oxc::transformer::JsxOptions::enable(),
      },
      env,
      proposals: oxc::transformer::ProposalOptions::default(),
      helper_loader: options
        .helpers
        .map_or_else(HelperLoaderOptions::default, HelperLoaderOptions::from),
      plugins: oxc::transformer::PluginsOptions::from(options.plugins.unwrap_or_default()),
      ..Default::default()
    })
  }
}
