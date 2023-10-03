use oxc::span::{Atom, Span};

#[derive(Debug)]
pub enum SourceMutation {
  RenameSymbol(Box<(Span, Atom)>),
  Remove(Box<Span>),
  AddExportDefaultBindingIdentifier(Box<Span>),
  AddNamespaceExport(),
}
