use string_wizard::MagicString;

use crate::PluginIdx;

#[derive(Debug)]
pub enum SourceMapGenMsg {
  MagicString(Box<(crate::ModuleIdx, PluginIdx, MagicString<'static>)>),
  Terminate,
}
