use oxc::{formatter::Gen, span::Atom};
use rolldown_common::{Part, SymbolRef};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use super::NormalModule;
use crate::bundler::{
  graph::symbols::Symbols, options::normalized_input_options::NormalizedInputOptions,
};

#[derive(Debug)]
pub struct RenderModuleContext<'a> {
  pub part: &'a Part,
  pub symbols: &'a Symbols,
  pub final_names: &'a FxHashMap<SymbolRef, Atom>,
  pub input_options: &'a NormalizedInputOptions,
}

impl NormalModule {
  pub fn render(&self, ctx: RenderModuleContext<'_>) -> Option<MagicString<'static>> {
    let mut formatter = oxc::formatter::Formatter::new(0, Default::default());
    let program = self.ast.program();
    let mut i = ctx.part.start;
    while i < ctx.part.end {
      let stmt = &program.body[i];
      if !matches!(stmt, oxc::ast::ast::Statement::EmptyStatement(_)) {
        stmt.gen(&mut formatter);
      }
      i += 1;
    }
    // let code = formatter.build(self.ast.program());
    let code = formatter.into_code();
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
