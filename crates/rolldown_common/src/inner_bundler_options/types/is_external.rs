use std::fmt::Debug;
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;

type Inner = dyn Fn(
    &str,         // specifier
    Option<&str>, // importer
    bool,         // is_resolved
  ) -> Pin<Box<(dyn Future<Output = anyhow::Result<bool>> + Send + 'static)>>
  + Send
  + Sync
  + 'static;

pub struct IsExternal(Box<Inner>);

impl Deref for IsExternal {
  type Target = Inner;

  fn deref(&self) -> &Self::Target {
    &*self.0
  }
}

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
    Self(Box::new(f))
  }

  pub fn from_vec(value: Vec<String>) -> Self {
    Self::from_closure(move |source, _, _| {
      let result = value.iter().any(|item| item == source);
      Box::pin(async move { Ok(result) })
    })
  }
}

impl From<Vec<String>> for IsExternal {
  fn from(value: Vec<String>) -> Self {
    IsExternal::from_vec(value)
  }
}

impl Debug for IsExternal {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "IsExternal(...)")
  }
}
