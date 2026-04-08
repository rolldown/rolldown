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
  /// Signal that a non-MagicString plugin intervened for this module.
  /// The background thread must flush any in-progress chain for the module
  /// before the next MagicString arrives, even if the code is unchanged.
  Barrier(crate::ModuleIdx),
  Terminate,
}
