use rolldown_common::SymbolRef;
use rolldown_rstr::Rstr;

#[derive(Debug)]
pub struct NamespaceAlias {
  pub property_name: Rstr,
  pub namespace_ref: SymbolRef,
}
