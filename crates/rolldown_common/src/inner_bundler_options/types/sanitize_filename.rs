use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc};

use arcstr::ArcStr;

type SanitizeFileNameFunction = dyn Fn(&str) -> Pin<Box<(dyn Future<Output = anyhow::Result<String>> + Send + 'static)>>
  + Send
  + Sync;

#[derive(Clone)]
pub enum SanitizeFilename {
  Boolean(bool),
  Fn(Arc<SanitizeFileNameFunction>),
}

impl Debug for SanitizeFilename {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Boolean(value) => write!(f, "SanitizeFileName::Boolean({value:?})"),
      Self::Fn(_) => write!(f, "SanitizeFileName::Fn(...)"),
    }
  }
}

impl Default for SanitizeFilename {
  fn default() -> Self {
    Self::Boolean(true)
  }
}

impl SanitizeFilename {
  pub async fn call(&self, name: &str) -> anyhow::Result<ArcStr> {
    match self {
      Self::Boolean(value) => {
        if *value {
          Ok(sanitize_filename::sanitize(name).into())
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
          sanitize_filename::sanitize(name).into()
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
