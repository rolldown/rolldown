use oxc::span::CompactStr;

use crate::SymbolRef;

#[derive(Debug, Clone)]
pub struct NamespaceAlias {
  pub property_name: CompactStr,
  pub namespace_ref: SymbolRef,
}
