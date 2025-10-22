use derive_more::Debug;
use rolldown_error::SingleBuildResult;
use std::{future::Future, pin::Pin, sync::Arc};

use arcstr::ArcStr;
use rolldown_utils::sanitize_filename::default_sanitize_file_name;

type SanitizeFileNameFunction = dyn Fn(&str) -> Pin<Box<dyn Future<Output = SingleBuildResult<String>> + Send + 'static>>
  + Send
  + Sync;

#[derive(Clone, Debug)]
pub enum SanitizeFilename {
  #[debug("SanitizeFileName::Boolean({})", "{0:?}")]
  Boolean(bool),
  #[debug("SanitizeFileName::Fn(...)")]
  Fn(Arc<SanitizeFileNameFunction>),
}

impl Default for SanitizeFilename {
  fn default() -> Self {
    Self::Boolean(true)
  }
}

impl SanitizeFilename {
  pub async fn call(&self, name: &str) -> SingleBuildResult<ArcStr> {
    match self {
      Self::Boolean(value) => {
        if *value {
          Ok(default_sanitize_file_name(name).into())
        } else {
          Ok(name.into())
        }
      }
      Self::Fn(value) => value(name).await.map(Into::into),
    }
  }

  pub fn value(&self, name: &str, fn_sanitized_file_name: Option<String>) -> ArcStr {
    match self {
      Self::Boolean(value) => {
        if *value {
          default_sanitize_file_name(name).into()
        } else {
          name.into()
        }
      }
      Self::Fn(_) => fn_sanitized_file_name.expect("SanitizeFilename Fn should has value").into(),
    }
  }
}

impl From<bool> for SanitizeFilename {
  fn from(value: bool) -> Self {
    Self::Boolean(value)
  }
}
