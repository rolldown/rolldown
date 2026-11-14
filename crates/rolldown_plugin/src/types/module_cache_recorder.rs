use std::sync::{
  Arc, Mutex,
  atomic::{AtomicBool, Ordering},
};

use arcstr::ArcStr;
use rolldown_common::{
  EmittedAsset, EmittedChunk, EmittedPrebuiltChunk, Log, LogLevel, side_effects::HookSideEffects,
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
  /// See `ignore_next_hook_side_effects`.
  ignore_next_hook_side_effects: AtomicBool,
}

#[derive(Debug, Default)]
pub struct RecordedEffects {
  /// In call order. Replaying keeps the order, so emitted files get the same names.
  pub actions: Vec<RecordedAction>,
  /// The last `moduleSideEffects` that a `load` or `transform` hook returned.
  pub hook_side_effects: Option<HookSideEffects>,
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
  /// A JavaScript hook returned `meta` with this top-level key for the module `module_id`. One
  /// action for each key. `module_id` is the key of the module in `moduleOptionMap` on the
  /// JavaScript side. `json` is the JSON text of the value, or `None` for the value `undefined`.
  JsModuleMeta {
    module_id: ArcStr,
    key: String,
    json: Option<String>,
  },
  /// A JavaScript hook returned `moduleSideEffects` for the module `module_id`, and the
  /// JavaScript side wrote it into `moduleOptionMap`. Rolldown's own copy of the value is in
  /// `RecordedEffects::hook_side_effects` or in the resolve results.
  JsModuleSideEffects {
    module_id: ArcStr,
    value: HookSideEffects,
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
      ignore_next_hook_side_effects: AtomicBool::new(false),
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

  /// The next `record_hook_side_effects` call does nothing.
  ///
  /// The wrapper of a JavaScript `load` or `transform` hook calls it when the hook returned no
  /// `moduleSideEffects`, and the wrapper passes on the value that an earlier `load` or
  /// `transform` hook of this module task returned. The saved result already holds that value.
  /// When another module task set the value, the wrapper marks the task not cacheable instead.
  /// The `load` and `transform` calls of a task run one after another, so the flag belongs to
  /// one call.
  pub fn ignore_next_hook_side_effects(&self) {
    self.ignore_next_hook_side_effects.store(true, Ordering::Relaxed);
  }

  pub fn record_hook_side_effects(&self, value: HookSideEffects) {
    if self.ignore_next_hook_side_effects.swap(false, Ordering::Relaxed) {
      return;
    }
    if let Ok(mut effects) = self.effects.lock() {
      effects.hook_side_effects = Some(value);
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

#[cfg(test)]
mod tests {
  use rolldown_common::side_effects::HookSideEffects;

  use super::ModuleCacheRecorder;

  #[test]
  fn ignores_only_the_next_hook_side_effects() {
    let recorder = ModuleCacheRecorder::new("a.js".into());
    recorder.record_hook_side_effects(HookSideEffects::False);
    recorder.ignore_next_hook_side_effects();
    recorder.record_hook_side_effects(HookSideEffects::True);
    assert_eq!(recorder.take_effects().hook_side_effects, Some(HookSideEffects::False));

    recorder.record_hook_side_effects(HookSideEffects::NoTreeshake);
    assert_eq!(recorder.take_effects().hook_side_effects, Some(HookSideEffects::NoTreeshake));
  }
}
