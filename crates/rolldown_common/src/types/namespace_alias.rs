use rolldown_rstr::Rstr;

use crate::{ImportOrExportName, SymbolRef};

#[derive(Debug)]
pub struct NamespaceAlias {
  pub property_name: ImportOrExportName,
  pub namespace_ref: SymbolRef,
}
