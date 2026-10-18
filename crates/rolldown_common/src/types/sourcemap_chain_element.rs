use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use arcstr::ArcStr;
use rolldown_sourcemap::SourceMap;
use string_wizard::{MagicString, SourceMapOptions};

use crate::PluginIdx;

#[derive(Debug, Clone)]
pub enum SourcemapChainElement {
  /// A string representing the URL of an external source map.
  Transform((PluginIdx, SourceMap)),
  /// A transform hook returned a native MagicString while
  /// `experimental.nativeMagicString` is enabled. The sourcemap worker
  /// generates the map, unless a later hook's `getCombinedSourcemap` needs it
  /// first.
  MagicString((PluginIdx, Arc<PendingSourcemap>)),
  /// An inline source map represented as a JSON string.
  Load(SourceMap),
  /// A transform hook returned changed code without a sourcemap.
  Omitted { plugin_idx: PluginIdx, plugin_name: ArcStr },
  /// A transform hook returned changed code together with an explicit
  /// `map: null`.
  Null { plugin_idx: PluginIdx, original_content: ArcStr },
}

/// The sourcemap of a native MagicString, generated on first use.
///
/// Until the map exists, only one thread uses this at a time, so `get` never
/// waits for another thread. That matters because the JS thread may be the
/// main thread of a browser page, where waiting traps. While the module's
/// transform hooks run, only the JS thread can reach it, through
/// `getCombinedSourcemap`. The plugin driver hands it to the sourcemap worker
/// only after the last hook has returned, and the worker is joined before
/// anything else reads it.
#[derive(Debug)]
pub struct PendingSourcemap {
  source: ArcStr,
  /// Taken by the first `get`, so the MagicString is dropped once the map exists.
  magic_string: Mutex<Option<MagicString<'static>>>,
  map: OnceLock<SourceMap>,
}

impl PendingSourcemap {
  pub fn new(source: ArcStr, magic_string: MagicString<'static>) -> Self {
    Self { source, magic_string: Mutex::new(Some(magic_string)), map: OnceLock::new() }
  }

  pub fn get(&self) -> &SourceMap {
    self.map.get_or_init(|| {
      // Only this initializer locks it, and only one initializer runs at a time.
      let magic_string = self
        .magic_string
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .take()
        .expect("PendingSourcemap: the MagicString is only taken by the first `get`");
      magic_string
        .source_map(SourceMapOptions { source: self.source.as_str().into(), ..Default::default() })
    })
  }

  pub fn is_generated(&self) -> bool {
    self.map.get().is_some()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn get_generates_the_map_once_and_drops_the_magic_string() {
    let mut magic_string = MagicString::new("const a = 1;\n");
    magic_string.prepend("// header\n");
    let expected = magic_string
      .source_map(SourceMapOptions { source: "a.js".into(), ..Default::default() })
      .to_json_string();
    let pending = PendingSourcemap::new("a.js".into(), magic_string);

    assert!(!pending.is_generated());
    let first = pending.get();
    assert_eq!(first.to_json_string(), expected);
    assert!(pending.is_generated());
    assert!(std::ptr::eq(first, pending.get()));
    assert!(pending.magic_string.lock().unwrap().is_none());
  }
}
