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

impl From<TransformOptions> for oxc::transformer::TransformOptions {
  fn from(value: TransformOptions) -> Self {
    // let env = match options.target {
    //   Some(Either::A(s)) => EnvOptions::from_target(&s)?,
    //   Some(Either::B(list)) => EnvOptions::from_target_list(&list)?,
    //   _ => EnvOptions::default(),
    // };
    let env = oxc::transformer::EnvOptions::default();
    Self {
      // cwd: options.cwd.map(PathBuf::from).unwrap_or_default(),
      // assumptions: options.assumptions.map(Into::into).unwrap_or_default(),
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
            // return Err(format!("Invalid jsx option: `{s}`."));
            oxc::transformer::JsxOptions::default()
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
    }
  }
}
