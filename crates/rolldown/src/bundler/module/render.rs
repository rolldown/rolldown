use super::NormalModule;
use crate::bundler::{
  graph::symbols::Symbols, options::normalized_input_options::NormalizedInputOptions,
  source_mutations,
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
  pub fn render(&self, _ctx: RenderModuleContext<'_>) -> Option<MagicString<'_>> {
    let mut s = MagicString::new(self.ast.source());

    self.source_mutations.iter().for_each(|mutation| {
      mutation.apply(&source_mutations::Context {}, &mut s);
    });

    s.prepend(format!("// {}\n", self.resource_id.prettify()));

    // TODO trim
    if s.len() == 0 {
      None
    } else {
      Some(s)
    }
  }
}
