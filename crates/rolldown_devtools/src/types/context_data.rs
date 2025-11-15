use std::ops::Deref;

use crate::type_alias::ContextDataMap;

/// Used to store context data extracted from spans.
/// Data are injected into spans use keys prefixed with `CONTEXT_`. Like `CONTEXT_build_id`, `CONTEXT_session_id`, etc.
/// These data would be extracted by `DevtoolsLayer` and stored in this struct with removed `CONTEXT_` prefix.
#[derive(Debug)]
pub struct ContextData(pub(crate) ContextDataMap);

impl Deref for ContextData {
  type Target = ContextDataMap;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}
