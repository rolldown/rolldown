use itertools::Either;
use oxc::transformer::EnvOptions;

use super::{
  compiler_assumptions::CompilerAssumptions, decorator_options::DecoratorOptions,
  jsx_options::JsxOptions, typescript_options::TypeScriptOptions,
};

#[derive(Debug, Default, Clone)]
pub struct TransformOptions {
  pub lang: Option<String>,

  pub jsx: Option<Either<String, JsxOptions>>,

  pub target: Option<Either<String, Vec<String>>>,

  pub decorator: Option<DecoratorOptions>,

  pub typescript: Option<TypeScriptOptions>,

  pub assumptions: Option<CompilerAssumptions>,
}

impl TryFrom<TransformOptions> for oxc::transformer::TransformOptions {
  type Error = String;

  fn try_from(value: TransformOptions) -> Result<Self, Self::Error> {
    let env = match value.target {
      Some(Either::Left(s)) => EnvOptions::from_target(&s)?,
      Some(Either::Right(list)) => EnvOptions::from_target_list(&list)?,
      _ => EnvOptions::default(),
    };
    Ok(Self {
      // cwd: options.cwd.map(PathBuf::from).unwrap_or_default(),
      assumptions: value.assumptions.map(Into::into).unwrap_or_default(),
      typescript: value
        .typescript
        .map(oxc::transformer::TypeScriptOptions::from)
        .unwrap_or_default(),
      decorator: value.decorator.map(oxc::transformer::DecoratorOptions::from).unwrap_or_default(),
      jsx: match value.jsx {
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
      ..Default::default()
    })
  }
}
