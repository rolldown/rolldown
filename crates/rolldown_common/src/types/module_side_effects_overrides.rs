use std::sync::{
  Arc,
  atomic::{AtomicBool, Ordering},
};

use arcstr::ArcStr;
use rolldown_utils::dashmap::FxDashMap;

use crate::{
  ImportAttributes, LogWithoutPlugin, ModuleInfo, ModuleInfoSideEffects, NormalizedBundlerOptions,
  SideEffectsWithoutHook,
  side_effects::{DeterminedSideEffects, HookSideEffects},
};

/// The warning for the first write to `moduleSideEffects` after `buildEnd`. The JS side builds
/// the same log in `logModuleSideEffectsWriteAfterBuildEnd`.
pub fn log_module_side_effects_write_after_build_end(module_id: &str) -> LogWithoutPlugin {
  LogWithoutPlugin {
    code: Some("MODULE_SIDE_EFFECTS_WRITE_AFTER_BUILD_END".to_string()),
    message: format!(
      "A write to `moduleSideEffects` of \"{module_id}\" after the `buildEnd` hook was ignored, because tree shaking has already used the value. Rolldown warns only about the first such write in a build."
    ),
    id: Some(module_id.to_string()),
    ..Default::default()
  }
}

pub type SharedModuleSideEffectsOverrides = Arc<ModuleSideEffectsOverrides>;

/// One write to `getModuleInfo(id).moduleSideEffects`. `None` is a write of `null` or
/// `undefined`: "not decided".
#[derive(Debug, Clone, Copy)]
pub struct WrittenSideEffects(pub Option<HookSideEffects>);

/// What a `treeshake.moduleSideEffects` function returned for one module. `None`: no boolean.
#[derive(Debug, Clone, Copy)]
pub struct OptionFnResult(pub Option<bool>);

/// The values that plugins write to `getModuleInfo(id).moduleSideEffects`, and the results of a
/// `treeshake.moduleSideEffects` function for the modules that needed it. JS writes here while
/// module tasks still run. See internal-docs/cache/implementation.md.
#[derive(Debug, Default)]
pub struct ModuleSideEffectsOverrides {
  values: FxDashMap<ArcStr, WrittenSideEffects>,
  option_fn_results: FxDashMap<ArcStr, OptionFnResult>,
  /// Without `experimental.importAttributes`, the key is the raw id, so one write reaches every
  /// external with that raw id.
  key_by_raw_id: bool,
  /// Tree shaking takes the writes after `buildEnd`, so a later write in the same build is
  /// ignored. Rolldown warns about the first one.
  build_end_done: AtomicBool,
  late_write_warned: AtomicBool,
}

impl ModuleSideEffectsOverrides {
  pub fn new(import_attributes_enabled: bool) -> Self {
    Self {
      values: FxDashMap::default(),
      option_fn_results: FxDashMap::default(),
      key_by_raw_id: !import_attributes_enabled,
      build_end_done: AtomicBool::new(false),
      late_write_warned: AtomicBool::new(false),
    }
  }

  /// All `buildEnd` hooks of the build finished.
  pub fn mark_build_end_done(&self) {
    self.build_end_done.store(true, Ordering::Release);
  }

  /// A scan starts, so writes reach tree shaking again.
  pub fn start_scan(&self) {
    self.build_end_done.store(false, Ordering::Release);
    self.late_write_warned.store(false, Ordering::Release);
  }

  fn key<'a>(&self, module_id: &'a str) -> &'a str {
    if self.key_by_raw_id {
      ImportAttributes::parse_module_id(module_id).map_or(module_id, |(raw_id, _)| raw_id)
    } else {
      module_id
    }
  }

  /// No plugin wrote a value.
  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }

  pub fn get(&self, module_id: &str) -> Option<WrittenSideEffects> {
    self.values.get(self.key(module_id)).map(|value| *value)
  }

  /// A write to `getModuleInfo(module_id).moduleSideEffects`. `None` is a write of `null` or
  /// `undefined`.
  ///
  /// A write after `buildEnd` is ignored: tree shaking has already used the values, and keeping
  /// the write would apply it in the next partial scan. Returns `true` for the first ignored write
  /// of a build, so the caller can warn.
  pub fn write(&self, module_id: &str, value: Option<HookSideEffects>) -> bool {
    if self.build_end_done.load(Ordering::Acquire) {
      return self
        .late_write_warned
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok();
    }
    self.values.insert(self.key(module_id).into(), WrittenSideEffects(value));
    false
  }

  pub fn remove_written(&self, module_id: &str) {
    self.values.remove(self.key(module_id));
  }

  /// The module will be fetched again, so it also calls the option function again when needed.
  pub fn remove(&self, module_id: &str) {
    let key = self.key(module_id);
    self.values.remove(key);
    self.option_fn_results.remove(key);
  }

  pub fn clear(&self) {
    self.values.clear();
    self.option_fn_results.clear();
  }

  /// The result of the `treeshake.moduleSideEffects` function for `module_id`, if it ran.
  pub fn option_fn_result(&self, module_id: &str) -> Option<OptionFnResult> {
    self.option_fn_results.get(self.key(module_id)).map(|value| *value)
  }

  /// Keeps the first result, so all readers see one value even if two calls overlap.
  pub fn set_option_fn_result(&self, module_id: &str, result: Option<bool>) -> Option<bool> {
    self.option_fn_results.entry(self.key(module_id).into()).or_insert(OptionFnResult(result)).0
  }

  /// The result of the `treeshake.moduleSideEffects` function for one module. It calls the
  /// function only when no earlier call for the module left a result. The caller makes sure the
  /// option is a function.
  pub async fn option_fn_result_or_call(
    &self,
    options: &NormalizedBundlerOptions,
    module_id: &str,
    raw_id: &str,
    external: bool,
  ) -> anyhow::Result<Option<bool>> {
    if let Some(OptionFnResult(result)) = self.option_fn_result(module_id) {
      return Ok(result);
    }
    let Some(treeshake) = options.treeshake.as_ref() else {
      return Ok(None);
    };
    let result = treeshake.module_side_effects.ffi_resolve(raw_id, external).await?;
    Ok(self.set_option_fn_result(module_id, result))
  }

  /// The values `getModuleInfo(module_id).moduleSideEffects` reads, with a written value and a
  /// known option function result applied.
  pub fn apply(
    &self,
    module_id: &str,
    side_effects: ModuleInfoSideEffects,
  ) -> ModuleInfoSideEffects {
    let decided = match self.get(module_id) {
      None => side_effects.decided,
      Some(WrittenSideEffects(written)) => written.map(HookSideEffects::declared),
    };
    let without_hook = match side_effects.without_hook {
      SideEffectsWithoutHook::OptionFn { fallback, .. } if decided.is_none() => self
        .option_fn_result(module_id)
        .map_or(side_effects.without_hook, |OptionFnResult(result)| {
          SideEffectsWithoutHook::Known(result.unwrap_or(fallback))
        }),
      without_hook => without_hook,
    };
    ModuleInfoSideEffects { decided, without_hook }
  }

  /// `None` when `info` has no side effects yet, or while the shown value needs an option function
  /// call that has not run.
  pub fn shown(&self, info: &ModuleInfo) -> Option<DeterminedSideEffects> {
    info.side_effects.and_then(|side_effects| self.apply(info.id.as_str(), side_effects).shown())
  }

  /// Like `shown`, but runs `call_option(raw_id, external)` when the shown value needs the
  /// option function result. The binding passes the JS function, which it can call at once.
  pub fn shown_or_call(
    &self,
    info: &ModuleInfo,
    call_option: impl FnOnce(&str, bool) -> anyhow::Result<Option<bool>>,
  ) -> anyhow::Result<Option<DeterminedSideEffects>> {
    let Some(side_effects) = info.side_effects else {
      return Ok(None);
    };
    let side_effects = self.apply(info.id.as_str(), side_effects);
    if let (None, SideEffectsWithoutHook::OptionFn { fallback, external }) =
      (side_effects.decided, side_effects.without_hook)
    {
      let result = call_option(info.id.raw_id(), external)?;
      let result = self.set_option_fn_result(info.id.as_str(), result);
      return Ok(Some(DeterminedSideEffects::UserDefined(result.unwrap_or(fallback))));
    }
    Ok(side_effects.shown())
  }

  /// Like `shown_or_call`, for Rust callers: it awaits the option function.
  pub async fn shown_or_call_async(
    &self,
    info: &ModuleInfo,
    options: &NormalizedBundlerOptions,
  ) -> anyhow::Result<Option<DeterminedSideEffects>> {
    let Some(side_effects) = info.side_effects else {
      return Ok(None);
    };
    let side_effects = self.apply(info.id.as_str(), side_effects);
    if let (None, SideEffectsWithoutHook::OptionFn { fallback, external }) =
      (side_effects.decided, side_effects.without_hook)
    {
      let result = self
        .option_fn_result_or_call(options, info.id.as_str(), info.id.raw_id(), external)
        .await?;
      return Ok(Some(DeterminedSideEffects::UserDefined(result.unwrap_or(fallback))));
    }
    Ok(side_effects.shown())
  }

  /// `info` with a written value and a known option function result applied. It copies `info`
  /// only when one of them changes it.
  pub fn apply_to_info(&self, info: Arc<ModuleInfo>) -> Arc<ModuleInfo> {
    let Some(side_effects) = info.side_effects else {
      return info;
    };
    let needs_option_fn_result = side_effects.decided.is_none()
      && matches!(side_effects.without_hook, SideEffectsWithoutHook::OptionFn { .. });
    if self.get(info.id.as_str()).is_none() && !needs_option_fn_result {
      return info;
    }
    let mut patched = ModuleInfo::clone(&info);
    patched.side_effects = Some(self.apply(info.id.as_str(), side_effects));
    Arc::new(patched)
  }
}
