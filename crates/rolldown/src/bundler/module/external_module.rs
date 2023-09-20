use index_vec::IndexVec;
use oxc::span::Atom;
use rolldown_common::{ImportRecord, ImportRecordId, ModuleId, ResourceId, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::bundler::graph::symbols::Symbols;

use super::{module::ModuleFinalizeContext, render::RenderModuleContext};

#[derive(Debug)]
pub struct ExternalModule {
  pub id: ModuleId,
  pub exec_order: u32,
  pub resource_id: ResourceId,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  pub is_symbol_for_namespace_referenced: bool,
  pub symbol_for_namespace: Option<(Atom, SymbolRef)>,
  pub symbols_imported_by_others: FxHashMap<Atom, SymbolRef>,
}

impl ExternalModule {
  pub fn new(id: ModuleId, resource_id: ResourceId) -> Self {
    Self {
      id,
      exec_order: u32::MAX,
      resource_id,
      import_records: Default::default(),
      is_symbol_for_namespace_referenced: false,
      symbol_for_namespace: None,
      symbols_imported_by_others: Default::default(),
    }
  }

  pub fn finalize(&mut self, _ctx: ModuleFinalizeContext) {}

  pub fn render(&self, ctx: RenderModuleContext) -> Option<MagicString<'static>> {
    let mut rendered = if let Some((symbol, symbol_ref)) = &self.symbol_for_namespace {
      let value = ctx
        .final_names
        .get(symbol_ref)
        .unwrap_or(symbol)
        .to_string();
      MagicString::new(format!(
        "import * as {value} from \"{}\"",
        self.resource_id.as_ref()
      ))
    } else if !self.symbols_imported_by_others.is_empty() {
      let specifiers = self
        .symbols_imported_by_others
        .iter()
        .map(|(imported, symbol)| ctx.final_names.get(symbol).unwrap_or(imported).to_string())
        .collect::<Vec<_>>()
        .join(", ");
      MagicString::new(format!(
        "import {{ {specifiers} }} from \"{}\"",
        self.resource_id.as_ref()
      ))
    } else {
      MagicString::new(format!("import \"{}\"", self.resource_id.as_ref()))
    };

    rendered.prepend(format!("// {}\n", self.resource_id.prettify()));
    rendered.append("\n");
    Some(rendered)
  }

  pub fn resolve_export(
    &mut self,
    symbols: &mut Symbols,
    exported: Atom,
    is_star: bool,
  ) -> SymbolRef {
    if is_star {
      if let Some((_, symbol_ref)) = &self.symbol_for_namespace {
        return *symbol_ref;
      }
      self.is_symbol_for_namespace_referenced = true;
      let symbol_ref: SymbolRef = (
        self.id,
        symbols.tables[self.id].create_symbol(exported.clone()),
      )
        .into();
      self.symbol_for_namespace = Some((exported.clone(), symbol_ref));
      symbol_ref
    } else {
      *self
        .symbols_imported_by_others
        .entry(exported.clone())
        .or_insert_with(|| {
          (
            self.id,
            symbols.tables[self.id].create_symbol(exported.clone()),
          )
            .into()
        })
    }
  }
}
