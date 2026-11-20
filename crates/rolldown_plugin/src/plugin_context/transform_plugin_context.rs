use std::{
  ops::Deref,
  sync::{Arc, Mutex, PoisonError, TryLockError},
};

use crate::PluginContext;
use arcstr::ArcStr;
use rolldown_common::{ModuleIdx, PendingSourcemap, SourcemapChainElement, WatchPath};
use rolldown_sourcemap::{SourceMap, collapse_sourcemaps_traced, empty_sourcemap};
use rolldown_utils::unique_arc::WeakRef;
use string_wizard::{MagicString, SourceMapOptions};

#[derive(Debug)]
pub struct TransformPluginContext {
  pub inner: PluginContext,
  sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
  original_code: ArcStr,
  id: ArcStr,
  module_idx: ModuleIdx,
  /// Whether the sourcemap worker runs. If it does, `send_magic_string` leaves
  /// the map to it instead of generating the map on the JS thread.
  has_sourcemap_worker: bool,
  /// The map that `send_magic_string` registers during the hook. The plugin
  /// driver takes it right after the hook (`take_pending_sourcemap`), so a
  /// context that a plugin keeps does not keep the map alive.
  pending_sourcemap: Mutex<PendingSourcemapSlot>,
}

/// See `TransformPluginContext::take_pending_sourcemap`.
#[derive(Debug)]
enum PendingSourcemapSlot {
  /// The plugin driver did not close the slot yet. It holds the map that
  /// `send_magic_string` registered, if any.
  Open(Option<Arc<PendingSourcemap>>),
  /// The plugin driver took the map. A later `send_magic_string` fails.
  Closed,
}

const SEND_AFTER_HOOK_ERROR: &str =
  "TransformPluginContext: `sendMagicString` can only be called during the transform hook";

impl TransformPluginContext {
  pub fn new(
    inner: PluginContext,
    sourcemap_chain: WeakRef<Vec<SourcemapChainElement>>,
    original_code: ArcStr,
    id: ArcStr,
    module_idx: ModuleIdx,
    has_sourcemap_worker: bool,
  ) -> Self {
    Self {
      inner,
      sourcemap_chain,
      original_code,
      id,
      module_idx,
      has_sourcemap_worker,
      pending_sourcemap: Mutex::new(PendingSourcemapSlot::Open(None)),
    }
  }

  pub fn get_combined_sourcemap(&self) -> SourceMap {
    self.sourcemap_chain.with_inner(|sourcemap_chain| {
      let empty_map = empty_sourcemap();
      let chain: Vec<&SourceMap> = sourcemap_chain
        .iter()
        .filter_map(|element| match element {
          SourcemapChainElement::Transform((_, sourcemap))
          | SourcemapChainElement::Load(sourcemap) => Some(sourcemap),
          // Generates the map on this thread if no earlier call did. The
          // sourcemap worker only gets it after the module's last hook.
          SourcemapChainElement::MagicString((_, pending)) => Some(pending.get()),
          SourcemapChainElement::Omitted { .. } => Some(&empty_map),
          SourcemapChainElement::Null { .. } => None,
        })
        .collect();
      // Like Rollup, keep only the sources and names that a traced token references, but
      // return a load map that no transform has remapped as is.
      match chain.as_slice() {
        [] => self.create_sourcemap(),
        [load] if matches!(sourcemap_chain.first(), Some(SourcemapChainElement::Load(_))) => {
          (*load).clone()
        }
        // TODO Here could be cache result for pervious sourcemap_chain, only remapping new sourcemap chain
        _ => collapse_sourcemaps_traced(&chain),
      }
    })
  }

  fn create_sourcemap(&self) -> SourceMap {
    let magic_string = MagicString::new(self.original_code.as_str());
    magic_string.source_map(SourceMapOptions {
      hires: string_wizard::Hires::Boundary,
      include_content: true,
      source: self.id.as_str().into(),
    })
  }

  /// Add a file or a directory as a watch dependency.
  pub fn add_watch_file(&self, file: &str) {
    // Skip all operations for virtual modules (starting with \0)
    // Virtual modules can't be refetched from disk during HMR
    if self.id.starts_with('\0') {
      return;
    }

    // Add to global watch files
    self.inner.add_watch_file(file);

    // Add to this module's transform dependencies
    if let crate::PluginContext::Native(ctx) = &self.inner {
      if let Some(plugin_driver) = ctx.plugin_driver.upgrade() {
        plugin_driver
          .add_transform_dependency(self.module_idx, WatchPath::new(file, &ctx.options.cwd));
      }
    }
  }

  /// Returns the map as JSON if there is no sourcemap worker. Otherwise the map
  /// is generated later and `None` is returned.
  pub fn send_magic_string(
    &self,
    magic_string: MagicString<'static>,
  ) -> anyhow::Result<Option<String>> {
    if !self.has_sourcemap_worker {
      // Same resolution as `PendingSourcemap::get`.
      return Ok(Some(
        magic_string
          .source_map(SourceMapOptions {
            hires: magic_string.hires().unwrap_or(string_wizard::Hires::Boundary),
            ..SourceMapOptions::default()
          })
          .to_json_string(),
      ));
    }
    // The JS thread only tries the lock, so it never waits. The lock is busy only
    // while the plugin driver closes the slot, which ends the time for a send.
    let mut slot = match self.pending_sourcemap.try_lock() {
      Ok(slot) => slot,
      Err(TryLockError::Poisoned(err)) => err.into_inner(),
      Err(TryLockError::WouldBlock) => anyhow::bail!(SEND_AFTER_HOOK_ERROR),
    };
    let PendingSourcemapSlot::Open(pending) = &mut *slot else {
      anyhow::bail!(SEND_AFTER_HOOK_ERROR);
    };
    if pending.is_some() {
      anyhow::bail!(
        "TransformPluginContext: `sendMagicString` can only be called once per transform hook"
      );
    }
    *pending = Some(Arc::new(PendingSourcemap::new(self.id.clone(), magic_string)));
    Ok(None)
  }

  /// Closes the slot and returns the map that `send_magic_string` registered, if
  /// any. The plugin driver calls this right after the hook returns or fails,
  /// before it reads the result. From then on, the context keeps no reference to
  /// the map, and `send_magic_string` fails when the sourcemap worker runs. A send
  /// from a JS microtask that runs after the hook returns, but before this call,
  /// still lands in the slot.
  pub fn take_pending_sourcemap(&self) -> Option<Arc<PendingSourcemap>> {
    let mut slot = self.pending_sourcemap.lock().unwrap_or_else(PoisonError::into_inner);
    match std::mem::replace(&mut *slot, PendingSourcemapSlot::Closed) {
      PendingSourcemapSlot::Open(pending) => pending,
      PendingSourcemapSlot::Closed => None,
    }
  }
}

impl Deref for TransformPluginContext {
  type Target = PluginContext;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub type SharedTransformPluginContext = Arc<TransformPluginContext>;

#[cfg(test)]
mod tests {
  use rolldown_utils::unique_arc::UniqueArc;

  use super::*;

  fn context(
    chain: &UniqueArc<Vec<SourcemapChainElement>>,
    has_sourcemap_worker: bool,
  ) -> TransformPluginContext {
    TransformPluginContext::new(
      PluginContext::new_napi_context(),
      chain.weak_ref(),
      ArcStr::default(),
      "a.js".into(),
      ModuleIdx::new(0),
      has_sourcemap_worker,
    )
  }

  #[test]
  fn take_pending_sourcemap_moves_the_map_out_and_closes_the_slot() {
    let chain = UniqueArc::new(vec![]);
    let ctx = context(&chain, true);
    assert_eq!(ctx.send_magic_string(MagicString::new("a")).unwrap(), None);
    let err = ctx.send_magic_string(MagicString::new("b")).unwrap_err();
    assert!(err.to_string().contains("once per transform hook"), "{err}");

    let pending = ctx.take_pending_sourcemap().expect("the first send registered a map");
    // The context keeps no reference, so a context that a plugin keeps does not
    // keep the map alive.
    assert_eq!(Arc::strong_count(&pending), 1);

    let err = ctx.send_magic_string(MagicString::new("c")).unwrap_err();
    assert!(err.to_string().contains("during the transform hook"), "{err}");
    assert!(ctx.take_pending_sourcemap().is_none());
  }

  #[test]
  fn send_magic_string_without_worker_does_not_use_the_slot() {
    let chain = UniqueArc::new(vec![]);
    let ctx = context(&chain, false);
    // Without the worker, the map comes back as JSON, and the slot stays empty.
    assert!(ctx.send_magic_string(MagicString::new("a")).unwrap().is_some());
    assert!(ctx.take_pending_sourcemap().is_none());
    // The fallback does not check the slot, so a send after the slot closed also works.
    assert!(ctx.send_magic_string(MagicString::new("b")).unwrap().is_some());
  }
}
