use std::{future::Future, pin::Pin, sync::Arc};

use derive_more::Debug;

type Inner = dyn Fn(
    &str,         // specifier
    Option<&str>, // importer
    bool,         // is_resolved
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
  + Send
  + Sync
  + 'static;

#[derive(Clone, Default, Debug)]
#[debug("IsExternal(...)")]
// Shared async closure for determining if modules are external
pub struct IsExternal(Option<Arc<Inner>>);

impl IsExternal {
  pub fn from_closure<F>(f: F) -> Self
  where
    F: Fn(
        &str,         // specifier
        Option<&str>, // importer
        bool,         // is_resolved
      ) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
      + Send
      + Sync
      + 'static,
  {
    Self(Some(Arc::new(f)))
  }

  pub fn from_vec(value: Vec<String>) -> Self {
    Self::from_closure(move |source, _, _| {
      let result = value.iter().any(|item| item == source);
      Box::pin(async move { Ok(result) })
    })
  }

  pub async fn call(
    &self,
    specifier: &str,
    importer: Option<&str>,
    is_resolved: bool,
  ) -> anyhow::Result<bool> {
    Ok(if let Some(is_external) = &self.0 {
      is_external(specifier, importer, is_resolved).await?
    } else {
      false
    })
  }
}

impl From<Vec<String>> for IsExternal {
  fn from(value: Vec<String>) -> Self {
    IsExternal::from_vec(value)
  }
}
