use oxc::span::Atom;
use rolldown_common::SymbolRef;
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use super::NormalModule;
use crate::bundler::{
  graph::symbols::Symbols, options::normalized_input_options::NormalizedInputOptions,
};

pub struct RenderModuleContext<'a> {
  pub symbols: &'a Symbols,
  pub final_names: &'a FxHashMap<SymbolRef, Atom>,
  pub input_options: &'a NormalizedInputOptions,
}

impl NormalModule {
  pub fn render(&self, _ctx: RenderModuleContext<'_>) -> Option<MagicString<'static>> {
    let formatter = oxc::formatter::Formatter::new(0, Default::default());
    let code = formatter.build(self.ast.program());
    if code.is_empty() {
      None
    } else {
      let mut s = MagicString::<'static>::new(code);
      s.prepend("// ");
      s.prepend(self.resource_id.prettify().to_string());
      s.prepend("\n");
      Some(s)
    }
  }
}
