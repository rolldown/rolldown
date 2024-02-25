use oxc::span::Atom;
use rolldown_common::SymbolRef;

#[derive(Debug)]
pub struct NamespaceAlias {
  pub property_name: Atom,
  pub namespace_ref: SymbolRef,
}
