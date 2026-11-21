//! Process-wide build activity, used to tell when allocator memory can be reclaimed.
//! See internal-docs/memory-reclaim/implementation.md

use std::sync::{
  OnceLock,
  atomic::{AtomicUsize, Ordering},
};

static ACTIVE_BUILDS: AtomicUsize = AtomicUsize::new(0);
static MEMORY_RELEASED_HOOK: OnceLock<fn()> = OnceLock::new();

/// Runs when the last active build ends, and whenever build data is released later.
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

pub struct BuildActivity(());

impl BuildActivity {
  pub fn enter() -> Self {
    ACTIVE_BUILDS.fetch_add(1, Ordering::AcqRel);
    Self(())
  }
}

impl Drop for BuildActivity {
  fn drop(&mut self) {
    if ACTIVE_BUILDS.fetch_sub(1, Ordering::AcqRel) == 1 {
      notify_memory_released();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{BuildActivity, has_active_build};

  #[test]
  fn activity_is_visible_until_the_guard_drops() {
    let activity = BuildActivity::enter();
    assert!(has_active_build());
    drop(activity);
  }
}
