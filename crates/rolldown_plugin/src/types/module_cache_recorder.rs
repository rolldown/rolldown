use std::sync::{
  Arc, Mutex,
  atomic::{AtomicBool, Ordering},
};

use arcstr::ArcStr;
use rolldown_common::{
  EmittedAsset, EmittedChunk, EmittedPrebuiltChunk, JsModuleMeta, JsModuleMetaEntry, Log, LogLevel,
  side_effects::HookSideEffects,
};

pub type SharedModuleCacheRecorder = Arc<ModuleCacheRecorder>;

/// Collects what the hooks of one module task do, so that the persistent scan cache can save the
/// result of the module and do the same actions again when it reuses that result.
///
/// The plugin driver passes it to every `resolveId`, `resolveDynamicImport`, `load` and
/// `transform` call of the task, including nested `this.resolve` calls.
#[derive(Debug)]
pub struct ModuleCacheRecorder {
  /// The module of the task. The hooks of a nested `this.resolve` call belong to this task too,
  /// whatever importer they get.
  module_id: ArcStr,
  not_cacheable: AtomicBool,
  not_cacheable_reason: Mutex<Option<String>>,
  effects: Mutex<RecordedEffects>,
}

#[derive(Debug, Default)]
pub struct RecordedEffects {
  /// In call order. Replaying keeps the order, so emitted files get the same names.
  pub actions: Vec<RecordedAction>,
  /// The last `moduleSideEffects` that a `load` or `transform` hook returned.
  pub hook_side_effects: Option<HookSideEffects>,
  /// The `meta` that the JS `load` and `transform` hooks of the task returned, merged.
  pub js_meta: JsModuleMeta,
}

#[derive(Debug)]
pub enum RecordedAction {
  /// `this.addWatchFile` on a plain context: the file is only watched.
  WatchFile(ArcStr),
  /// `this.addWatchFile` on a `load` or `transform` context. Also recorded as `WatchFile`.
  TransformDependency(ArcStr),
  EmitFile {
    file: EmittedAsset,
    fn_asset_filename: Option<String>,
    fn_sanitized_file_name: Option<String>,
  },
  EmitFileAsync(EmittedAsset),
  EmitChunk(EmittedChunk),
  EmitPrebuiltChunk(EmittedPrebuiltChunk),
  AssociateModuleWithFileRef {
    module_id: ArcStr,
    reference_id: ArcStr,
  },
  /// A log from a native plugin, with the plugin name already filled in.
  Log {
    level: LogLevel,
    log: Log,
  },
  /// A log from a JavaScript plugin, as the JSON of the object that reached `onLog`.
  JsLog {
    level: LogLevel,
    json: String,
  },
  /// A Rust plugin set the value under `key` in its meta of the module `module_id`.
  RustModuleMeta {
    module_id: ArcStr,
    key: ArcStr,
    value: Arc<[u8]>,
  },
}

impl ModuleCacheRecorder {
  pub fn new(module_id: ArcStr) -> Self {
    Self {
      module_id,
      not_cacheable: AtomicBool::new(false),
      not_cacheable_reason: Mutex::new(None),
      effects: Mutex::new(RecordedEffects::default()),
    }
  }

  pub fn module_id(&self) -> &ArcStr {
    &self.module_id
  }

  /// The module gets no saved result. The first reason is kept for debugging.
  pub fn mark_not_cacheable(&self, reason: impl FnOnce() -> String) {
    if !self.not_cacheable.swap(true, Ordering::Relaxed) {
      if let Ok(mut slot) = self.not_cacheable_reason.lock() {
        *slot = Some(reason());
      }
    }
  }

  pub fn is_cacheable(&self) -> bool {
    !self.not_cacheable.load(Ordering::Relaxed)
  }

  pub fn not_cacheable_reason(&self) -> Option<String> {
    self.not_cacheable_reason.lock().ok().and_then(|slot| slot.clone())
  }

  pub fn record(&self, action: RecordedAction) {
    if !self.is_cacheable() {
      return;
    }
    if let Ok(mut effects) = self.effects.lock() {
      effects.actions.push(action);
    }
  }

  pub fn record_hook_side_effects(&self, value: HookSideEffects) {
    if let Ok(mut effects) = self.effects.lock() {
      effects.hook_side_effects = Some(value);
    }
  }

  pub fn record_js_meta(&self, entries: Vec<JsModuleMetaEntry>) {
    if let Ok(mut effects) = self.effects.lock() {
      effects.js_meta.merge(entries);
    }
  }

  /// The number of actions recorded so far. The module task reads it when `transform` has
  /// ended, to tell the actions of `load` and `transform` from the actions of the resolve hooks.
  pub fn action_count(&self) -> usize {
    self.effects.lock().map_or(0, |effects| effects.actions.len())
  }

  pub fn take_effects(&self) -> RecordedEffects {
    self.effects.lock().map(|mut effects| std::mem::take(&mut *effects)).unwrap_or_default()
  }
}
