use arcstr::ArcStr;
use string_wizard::MagicString;

use crate::PluginIdx;

#[derive(Debug)]
pub enum SourceMapGenMsg {
  /// `(module_idx, plugin_idx, module_id, magic_string)`.
  ///
  /// `module_id` is carried so the sourcemap worker can fill the generated
  /// map's `source` with it
  MagicString(Box<(crate::ModuleIdx, PluginIdx, ArcStr, MagicString<'static>)>),
  /// Close the in-progress chain of this module. Sent when a non-MagicString
  /// plugin intervened for the module, so the background thread flushes the
  /// chain before the next MagicString arrives, even if the code is unchanged.
  /// Also sent when the transform hooks of the module finish.
  Barrier(crate::ModuleIdx),
  Terminate,
}
