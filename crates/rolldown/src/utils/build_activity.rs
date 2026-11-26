use std::sync::{
  OnceLock,
  atomic::{AtomicUsize, Ordering},
};

// See internal-docs/memory-reclaim/implementation.md.
static ACTIVE_BUILDS: AtomicUsize = AtomicUsize::new(0);
static MEMORY_RELEASED_HOOK: OnceLock<fn()> = OnceLock::new();

pub fn set_memory_released_hook(hook: fn()) {
  let _ = MEMORY_RELEASED_HOOK.set(hook);
}

pub fn notify_memory_released() {
  if let Some(hook) = MEMORY_RELEASED_HOOK.get() {
    hook();
  }
}

pub fn has_active_build() -> bool {
  ACTIVE_BUILDS.load(Ordering::Acquire) > 0
}

pub struct BuildActivity;

impl BuildActivity {
  pub fn enter() -> Self {
    ACTIVE_BUILDS.fetch_add(1, Ordering::AcqRel);
    Self
  }
}

impl Drop for BuildActivity {
  fn drop(&mut self) {
    if ACTIVE_BUILDS.fetch_sub(1, Ordering::AcqRel) == 1 {
      notify_memory_released();
    }
  }
}

/// Dropped with the last holder of a build's data, the plugin driver or a plugin context JS keeps.
pub struct BuildData;

impl Drop for BuildData {
  fn drop(&mut self) {
    notify_memory_released();
  }
}
