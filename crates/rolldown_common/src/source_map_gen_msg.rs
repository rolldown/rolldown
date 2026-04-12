use string_wizard::MagicString;

use crate::PluginIdx;

#[derive(Debug)]
pub enum SourceMapGenMsg {
  MagicString(Box<(crate::ModuleIdx, PluginIdx, MagicString<'static>)>),
  /// Signal that a non-MagicString plugin intervened for this module.
  /// The background thread must flush any in-progress chain for the module
  /// before the next MagicString arrives, even if the code is unchanged.
  Barrier(crate::ModuleIdx),
  Terminate,
}
