use super::{source_mutation::SourceMutation, NormalModule};
use crate::bundler::{
  graph::symbols::{get_reference_final_name, get_symbol_final_name, Symbols},
  options::normalized_input_options::NormalizedInputOptions,
};
use oxc::span::Atom;
use rolldown_common::SymbolRef;
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

#[derive(Debug)]
pub struct RenderModuleContext<'a> {
  pub symbols: &'a Symbols,
  pub final_names: &'a FxHashMap<SymbolRef, Atom>,
  pub input_options: &'a NormalizedInputOptions,
}

impl NormalModule {
  pub fn render(&self, ctx: RenderModuleContext<'_>) -> Option<MagicString<'_>> {
    let mut s = MagicString::new(self.ast.source());

    s.prepend(format!("// {}\n", self.resource_id.prettify()));

    for mutation in &self.source_mutations {
      match mutation {
        SourceMutation::RenameSymbol(r) => {
          s.update(r.0.start as usize, r.0.end as usize, r.1.as_str());
        }
        SourceMutation::Remove(span) => {
          s.remove(span.start as usize, span.end as usize);
        }
        SourceMutation::AddExportDefaultBindingIdentifier(span) => {
          if let Some(name) = get_symbol_final_name(
            self.id,
            self.default_export_symbol.unwrap(),
            ctx.symbols,
            ctx.final_names,
          ) {
            s.update(
              span.start as usize,
              span.end as usize,
              format!("var {name} = "),
            );
          }
        }
        SourceMutation::AddNamespaceExport() => {
          if let Some(name) = get_symbol_final_name(
            self.id,
            self.namespace_symbol.0.symbol,
            ctx.symbols,
            ctx.final_names,
          ) {
            let exports = self
              .resolved_exports
              .iter()
              .map(|(name, info)| {
                format!(
                  "  get {name}() {{ return {} }}",
                  if let Some(name) =
                    get_reference_final_name(self.id, info.local_ref, ctx.symbols, ctx.final_names,)
                  {
                    name
                  } else {
                    name
                  }
                )
              })
              .collect::<Vec<_>>()
              .join(",\n");
            s.append(format!("\nvar {name} = {{\n{exports}\n}};\n"));
          }
        }
      }
    }

    // TODO trim
    if s.len() == 0 {
      None
    } else {
      Some(s)
    }
  }
}
