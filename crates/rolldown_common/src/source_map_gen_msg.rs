use string_wizard::MagicString;

#[derive(Debug)]
pub enum SourceMapGenMsg {
  MagicString(Box<(crate::ModuleIdx, u32, MagicString<'static>)>),
  Terminate,
}
