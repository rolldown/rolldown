#[cfg(feature = "deserialize_bundler_options")]
use schemars::JsonSchema;
#[cfg(feature = "deserialize_bundler_options")]
use serde::Deserialize;

use std::{
  fmt::Debug,
  future::{self, Future},
  ops::Deref,
  pin::Pin,
};

type Inner = dyn Fn(
    &str, // module id
  )
    -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<ManualChunksMatch>>> + Send + 'static)>>
  + Send
  + Sync
  + 'static;

pub struct ManualChunksMatch {
  // Chunk name
  pub name: String,
  pub include_dependencies_recursively: bool,
}

#[cfg_attr(
  feature = "deserialize_bundler_options",
  derive(Deserialize, JsonSchema),
  serde(rename_all = "camelCase", deny_unknown_fields)
)]
pub struct ManualChunksDescriptor {
  pub filter: String,
  pub name: String,
  pub include_dependencies_recursively: Option<bool>,
  pub priority: Option<u32>,
}

pub struct ManualChunksOption(Box<Inner>);

impl Debug for ManualChunksOption {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("ManualChunksOption").finish()
  }
}

impl Deref for ManualChunksOption {
  type Target = Inner;

  fn deref(&self) -> &Self::Target {
    &*self.0
  }
}

impl ManualChunksOption {
  pub fn from_vec(mut vec: Vec<ManualChunksDescriptor>) -> Self {
    // Notice that we rely on stable sort here
    vec.sort_by_key(|x| x.priority.unwrap_or(0));
    Self::from_closure(move |module_id| {
      Box::pin(future::ready(Ok(vec.iter().find_map(|desc| {
        if module_id.contains(&desc.filter) {
          Some(ManualChunksMatch {
            name: desc.name.clone(),
            include_dependencies_recursively: desc
              .include_dependencies_recursively
              .unwrap_or(false),
          })
        } else {
          None
        }
      }))))
    })
  }

  pub fn from_closure<F>(f: F) -> Self
  where
    F: Fn(
        &str, // module id
      ) -> Pin<
        Box<(dyn Future<Output = anyhow::Result<Option<ManualChunksMatch>>> + Send + 'static)>,
      > + Send
      + Sync
      + 'static,
  {
    Self(Box::new(f))
  }
}
