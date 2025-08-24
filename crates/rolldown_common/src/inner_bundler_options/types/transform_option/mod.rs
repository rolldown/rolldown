mod compiler_assumptions;
mod decorator_options;
mod jsx_options;
mod plugin_options;
mod typescript_options;

use oxc::transformer::EnvOptions;

pub use itertools::Either;
pub use {
  compiler_assumptions::CompilerAssumptions,
  decorator_options::DecoratorOptions,
  jsx_options::{JsxOptions, ReactRefreshOptions},
  plugin_options::{PluginsOptions, StyledComponentsOptions},
  typescript_options::{IsolatedDeclarationsOptions, TypeScriptOptions},
};

#[derive(Debug, Default, Clone)]
pub struct TransformOptions {
  /// Configure how TSX and JSX are transformed.
  pub jsx: Option<Either<String, JsxOptions>>,

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
      // helper_loader: options
      //   .helpers
      //   .map_or_else(HelperLoaderOptions::default, HelperLoaderOptions::from),
      plugins: options.plugins.map(oxc::transformer::PluginsOptions::from).unwrap_or_default(),
      ..Default::default()
    })
  }
}
