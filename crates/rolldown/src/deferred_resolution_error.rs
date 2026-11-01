/// A snapshot host has established ordinary absence, rather than an ownership,
/// policy, admission, configuration or plugin failure. The loader still decides
/// whether the graph permits deferring this error to a dynamic import.
#[derive(Debug)]
pub struct DeferredResolutionError {
  message: String,
}

impl DeferredResolutionError {
  pub fn new(message: String) -> Self {
    Self { message }
  }
}

impl std::fmt::Display for DeferredResolutionError {
  fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl std::error::Error for DeferredResolutionError {}

pub(crate) const FAILURE_MODULE_PREFIX: &str = "\0rolldown/deferred-resolution:";

pub(crate) fn is_link_failure(
  modules: &oxc_index::IndexVec<rolldown_common::ModuleIdx, rolldown_common::Module>,
  idx: rolldown_common::ModuleIdx,
) -> bool {
  let Some(module) = modules[idx].as_normal() else { return false };
  module.named_imports.values().any(|import| {
    matches!(&import.imported, rolldown_common::Specifier::Literal(name) if name == "__moduleLinkError")
      && module.import_records[import.record_idx]
        .resolved_module
        .is_some_and(|target| modules[target].id().as_str() == rolldown_common::RUNTIME_MODULE_KEY)
  })
}

pub(crate) fn failure_message(
  modules: &oxc_index::IndexVec<rolldown_common::ModuleIdx, rolldown_common::Module>,
  idx: rolldown_common::ModuleIdx,
) -> Option<String> {
  let module = modules[idx].as_normal()?;
  let direct = module.id.as_str().starts_with(FAILURE_MODULE_PREFIX);
  let failure = if direct {
    module
  } else {
    if module.import_records.len() != 1 {
      return None;
    }
    let record = module.import_records.iter().next()?;
    if record.kind != rolldown_common::ImportKind::Import {
      return None;
    }
    modules[record.resolved_module?].as_normal()?
  };
  let encoded =
    failure.id.as_str().strip_prefix(FAILURE_MODULE_PREFIX)?.strip_suffix("/failure.mjs")?;
  let (_, request): (String, String) = serde_json::from_str(encoded).ok()?;
  // A bare specifier in a static dependency fails that module's link phase.
  // Keep its ordinary initializer/error cache; only direct import() is fresh.
  if !direct && !request.starts_with("./") {
    return None;
  }
  Some(format!("Failed to resolve module {request:?}"))
}
