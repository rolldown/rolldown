use oxc::{
  ast::{
    AstBuilder,
    ast::{
      BindingPatternKind, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
      ExportNamedDeclaration,
    },
  },
  ast_visit::Visit,
};
use rolldown_common::{ConstExportMeta, GetLocalDb, SymbolRef};
use rustc_hash::FxHashMap;

use crate::ast_scanner::const_eval::{ConstEvalCtx, try_extract_const_literal};

use super::LinkStage;

impl LinkStage<'_> {
  pub(super) fn cross_module_optimization(&mut self) {
    if self.options.optimization.inline_const_pass() < 2 {
      return;
    }
    // Explain `inline_const.pass`:
    // - if `inline_const.pass` is 1, we don't need the extra visit pass, since we already do it in
    // scan phase. This would already cover most of the cases, and the overhead is minimal.
    // - if `inline_const.pass` is greater than 1, and there is no cycle in module graph,
    // we could just revisit the ast of module in topological order only once.
    // - TODO:
    //  if there is cycle in module graph, and the `inline_const.pass` is greater than `1`, we
    //  should revisit the ast of the module for `inline_const.pass - 1` time.
    //  potential optimization:
    //  - if in one pass there is no new constant export found, we can stop the pass early.
    //  - if all dependencies of a module has no constant export, we don't need to visit ast at all.
    // The extra passes only when user enable `inline_const` and set `pass` greater than 1.
    let mut constant_symbol_map = std::mem::take(&mut self.constant_symbol_map);
    for module in self.sorted_modules.iter().filter_map(|item| self.module_table[*item].as_normal())
    {
      let (ast, module_idx) =
        self.ast_table.get(module.ecma_ast_idx()).expect("ecma ast should be set");
      ast.program.with_dependent(|owner, dep| {
        let module_symbol_table = self.symbols.local_db(*module_idx);
        let eval_ctx = ConstEvalCtx {
          ast: AstBuilder::new(&owner.allocator),
          scope: module_symbol_table.scoping(),
          module_idx: module.idx,
          f: &|reference_id, module_idx| {
            let reference = module_symbol_table.scoping().get_reference(reference_id);
            let symbol_id = reference.symbol_id()?;
            let symbol_ref: SymbolRef = (module_idx, symbol_id).into();
            let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
            constant_symbol_map.get(&canonical_ref).map(|meta| meta.value.clone())
          },
        };
        let mut ctx = Context::new(eval_ctx, module.default_export_ref);
        ctx.visit_program(&dep.program);
        constant_symbol_map.extend(ctx.local_symbol_map);
      });
    }
    self.constant_symbol_map = constant_symbol_map;
  }
}

struct Context<'a, 'ast: 'a> {
  local_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  eval_ctx: ConstEvalCtx<'a, 'ast>,
  export_default_symbol: SymbolRef,
}

impl<'a, 'ast: 'a> Context<'a, 'ast> {
  fn new(eval_ctx: ConstEvalCtx<'a, 'ast>, export_default_symbol: SymbolRef) -> Self {
    Self { local_symbol_map: FxHashMap::default(), eval_ctx, export_default_symbol }
  }
}

impl<'a, 'ast: 'a> Visit<'ast> for Context<'a, 'ast> {
  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'ast>) {
    if it.source.is_some() {
      return;
    }

    let Some(ref decl) = it.declaration else {
      return;
    };

    let Declaration::VariableDeclaration(var_decl) = decl else {
      return;
    };

    var_decl.declarations.iter().for_each(|declarator| {
      if let BindingPatternKind::BindingIdentifier(ref binding) = declarator.id.kind
        && let Some(value) =
          declarator.init.as_ref().and_then(|expr| try_extract_const_literal(&self.eval_ctx, expr))
      {
        let symbol_ref: SymbolRef = (self.eval_ctx.module_idx, binding.symbol_id()).into();
        self.local_symbol_map.insert(symbol_ref, ConstExportMeta::new(value, false));
      }
    });
  }
  fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'ast>) {
    let Some(expr) = it.declaration.as_expression() else {
      return;
    };
    let local_binding_for_default_export = match &it.declaration {
      oxc::ast::match_expression!(ExportDefaultDeclarationKind) => None,
      ExportDefaultDeclarationKind::FunctionDeclaration(fn_decl) => {
        fn_decl.id.as_ref().map(rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id)
      }
      ExportDefaultDeclarationKind::ClassDeclaration(cls_decl) => {
        cls_decl.id.as_ref().map(rolldown_ecmascript_utils::BindingIdentifierExt::expect_symbol_id)
      }
      ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => unreachable!(),
    };

    let symbol_id = local_binding_for_default_export.unwrap_or(self.export_default_symbol.symbol);
    let symbol_ref: SymbolRef = (self.eval_ctx.module_idx, symbol_id).into();
    if let Some(v) = try_extract_const_literal(&self.eval_ctx, expr) {
      self.local_symbol_map.insert(symbol_ref, ConstExportMeta::new(v, false));
    }
  }
}
