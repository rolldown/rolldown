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
  Terminate,
}
