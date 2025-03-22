use rolldown_rstr::Rstr;

use crate::SymbolRef;

#[derive(Debug, Clone)]
pub struct NamespaceAlias {
  pub property_name: Rstr,
  pub namespace_ref: SymbolRef,
}
