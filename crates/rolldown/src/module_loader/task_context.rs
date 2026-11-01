use oxc::transformer_plugins::ReplaceGlobalDefinesConfig;
use rolldown_common::{ModuleId, ModuleIdx, ModuleLoaderMsg};
use rolldown_error::BuildDiagnostic;
use rolldown_fs::FileSystem;
use rolldown_plugin::SharedPluginDriver;
use rustc_hash::FxHashMap;

use crate::{SharedOptions, SharedResolver};

/// A deferred resolution's diagnostic inputs, from which a [`BuildDiagnostic`] can be built more
/// than once across scans (#700).
#[derive(Clone, Debug)]
pub struct DeferredResolutionDiagnostic {
  pub source: arcstr::ArcStr,
  pub importer: String,
  pub message: String,
  pub span: Option<oxc::span::Span>,
}

impl DeferredResolutionDiagnostic {
  pub fn into_diagnostic(self) -> BuildDiagnostic {
    BuildDiagnostic::oxc_error(
      self.source,
      self.importer,
      String::new(),
      self.message,
      self
        .span
        .into_iter()
        .map(|span| oxc::diagnostics::LabeledSpan::at(span, "unresolved snapshot dependency"))
        .collect(),
      rolldown_error::EventKind::UnresolvedImport,
    )
  }
}

/// Used to store common data shared between all tasks.
pub struct TaskContext<Fs: FileSystem> {
  pub options: SharedOptions,
  pub tx: tokio::sync::mpsc::UnboundedSender<ModuleLoaderMsg>,
  pub resolver: SharedResolver<Fs>,
  pub fs: Fs,
  pub plugin_driver: SharedPluginDriver,
  pub meta: TaskContextMeta,
  pub deferred_parse_errors: std::sync::Mutex<FxHashMap<ModuleIdx, Vec<BuildDiagnostic>>>,
  pub deferred_resolution_targets:
    std::sync::Mutex<FxHashMap<ModuleId, DeferredResolutionDiagnostic>>,
  /// Whether module tasks right-size the arenas of the ASTs and scope trees that an incremental
  /// build retains ([`crate::utils::retained_memory`]).
  pub compact_module_arenas: bool,
}

pub struct TaskContextMeta {
  pub replace_global_define_config: Option<ReplaceGlobalDefinesConfig>,
}

impl<Fs: FileSystem> TaskContext<Fs> {
  pub fn send(&self, message: ModuleLoaderMsg) {
    if self.tx.send(message).is_err() {
      tracing::debug!("discarding module task result after scan cancellation");
    }
  }
}
