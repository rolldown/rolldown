use arcstr::ArcStr;

use super::BuildEvent;
use crate::{types::diagnostic_options::DiagnosticOptions, types::event_kind::EventKind};

/// An external module that a module in the dependencies of an `import defer` target imports.
/// The output format loads it when the chunk that imports it loads.
#[derive(Debug)]
pub struct DeferredExternalEvaluatedEagerly {
  pub external_id: ArcStr,
  /// The module with the `import defer` declaration or the `import.defer()` call.
  pub deferring_module: ArcStr,
  /// The target of `import defer` or `import.defer()`.
  pub deferred_module: ArcStr,
  /// The module that imports the external module. It is `deferred_module` or a module in its
  /// dependencies.
  pub importer: ArcStr,
  /// `ESM`, `IIFE` or `UMD`.
  pub format: &'static str,
}

impl BuildEvent for DeferredExternalEvaluatedEagerly {
  fn kind(&self) -> EventKind {
    EventKind::DeferredExternalEvaluatedEagerly
  }

  fn id(&self) -> Option<String> {
    Some(self.importer.to_string())
  }

  fn ids(&self) -> Option<Vec<String>> {
    Some(vec![
      self.deferring_module.to_string(),
      self.deferred_module.to_string(),
      self.importer.to_string(),
    ])
  }

  fn message(&self, opts: &DiagnosticOptions) -> String {
    let external = &self.external_id;
    let deferring = opts.stabilize_path(self.deferring_module.as_str());
    let deferred = opts.stabilize_path(self.deferred_module.as_str());
    let importer = opts.stabilize_path(self.importer.as_str());
    let path = if importer == deferred {
      format!("\"{deferring}\" defers \"{deferred}\", and \"{deferred}\" imports \"{external}\".")
    } else {
      format!(
        "\"{deferring}\" defers \"{deferred}\", and \"{importer}\" in the dependencies of \"{deferred}\" imports \"{external}\"."
      )
    };
    format!(
      "The external module \"{external}\" evaluates when its chunk loads, not when the deferred module evaluates. {path} {} output loads each external module at the start of the chunk that imports it. To evaluate \"{external}\" lazily, bundle it, or use the CommonJS output format, which loads it with `require()` in the deferred code. CommonJS output works only when the runtime supports CommonJS and the module can load with `require()`.",
      self.format,
    )
  }
}
