use oxc::{
  allocator::Address,
  ast::{
    AstBuilder,
    ast::{
      BindingPatternKind, Declaration, ExportDefaultDeclaration, ExportDefaultDeclarationKind,
      ExportNamedDeclaration,
    },
  },
  ast_visit::{Visit, walk},
  semantic::ScopeFlags,
};
use rolldown_common::{
  AstScopes, ConstExportMeta, EcmaViewMeta, GetLocalDb, ModuleIdx, SharedNormalizedBundlerOptions,
  SideEffectDetail, StmtInfoIdx, SymbolRef, SymbolRefDb, SymbolRefFlags,
};
use rolldown_ecmascript_utils::{ExpressionExt, is_top_level};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  ast_scanner::{
    const_eval::{ConstEvalCtx, try_extract_const_literal},
    side_effect_detector::{FlatOptions, SideEffectDetector},
  },
  module_finalizers::TraverseState,
};

use super::LinkStage;

#[derive(Default)]
struct CrossModuleInlineConstCtx {
  changed: bool,
  config: CrossModuleOptimizationConfig,
}

impl CrossModuleInlineConstCtx {
  fn new(config: CrossModuleOptimizationConfig) -> Self {
    Self { changed: true, config }
  }
}

#[derive(Default, Clone, Copy, Debug)]
struct CrossModuleOptimizationConfig {
  pass: u32,
  #[expect(unused)]
  side_effects_free_function_optimization: bool,
  inline_const_optimization: bool,
}

impl LinkStage<'_> {
  fn prepare_cross_module_optimization(&mut self) -> CrossModuleOptimizationConfig {
    let side_effect_free_function_symbols = self
      .module_table
      .iter()
      .zip(self.symbols.inner().iter())
      .filter_map(|(m, symbol_for_module)| {
        let normal_module = m.as_normal()?;
        let idx = normal_module.idx;
        normal_module
          .meta
          .contains(EcmaViewMeta::TopExportedSideEffectsFreeFunction)
          .then(move || {
            let symbol_for_module = symbol_for_module.as_ref()?;
            Some(symbol_for_module.flags.iter().filter_map(move |(symbol_id, flag)| {
              flag
                .contains(SymbolRefFlags::SideEffectsFreeFunction)
                .then_some(SymbolRef::from((idx, *symbol_id)))
            }))
          })
          .flatten()
      })
      .flatten()
      .collect::<FxHashSet<SymbolRef>>();
    self.side_effects_free_function_symbol_ref = side_effect_free_function_symbols;

    #[expect(clippy::bool_to_int_with_if)]
    let other_optimization_pass =
      if self.side_effects_free_function_symbol_ref.is_empty() { 0 } else { 1 };
    let inline_const_pass = self.options.optimization.inline_const_pass() - 1;
    CrossModuleOptimizationConfig {
      pass: inline_const_pass.max(other_optimization_pass),
      side_effects_free_function_optimization: !self
        .side_effects_free_function_symbol_ref
        .is_empty(),
      inline_const_optimization: self.options.optimization.is_inline_const_enabled(),
    }
  }
  pub(super) fn cross_module_optimization(&mut self) {
    let config = self.prepare_cross_module_optimization();
    if config.pass < 1 {
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
    // The extra passes only run when user enable `inline_const` and set `pass` greater than 1.
    let mut ctx = CrossModuleInlineConstCtx::new(config);
    let mut constant_symbol_map = std::mem::take(&mut self.global_constant_symbol_map);
    while ctx.config.pass > 0 && ctx.changed {
      ctx.config.pass -= 1;
      ctx.changed = false;
      self.run(&mut ctx, &mut constant_symbol_map);
      if !ctx.changed {
        break;
      }
    }
    self.global_constant_symbol_map = constant_symbol_map;
  }

  fn run(
    &mut self,
    cross_module_inline_const_ctx: &mut CrossModuleInlineConstCtx,
    constant_symbol_map: &mut FxHashMap<SymbolRef, ConstExportMeta>,
  ) {
    let mut side_effect_mutation_map: FxHashMap<ModuleIdx, Vec<(StmtInfoIdx, SideEffectDetail)>> =
      FxHashMap::default();
    for module in
      self.sorted_modules.iter_mut().filter_map(|item| self.module_table[*item].as_normal())
    {
      let module_idx = module.idx;
      let ast = self.ast_table[module_idx].as_ref().expect("ast should be set in a normal module");
      // A dummy map to fits the api of `ConstEvalCtx`
      let constant_map = FxHashMap::default();
      ast.program.with_dependent(|owner, dep| {
        let module_symbol_table = self.symbols.local_db(module_idx);
        let eval_ctx = ConstEvalCtx {
          ast: AstBuilder::new(&owner.allocator),
          scope: module_symbol_table.scoping(),
          overrode_get_constant_value_from_reference_id: Some(&|reference_id| {
            let reference = module_symbol_table.scoping().get_reference(reference_id);
            let symbol_id = reference.symbol_id()?;
            let symbol_ref: SymbolRef = (module_idx, symbol_id).into();
            let canonical_ref = self.symbols.canonical_ref_for(symbol_ref);
            constant_symbol_map
              .get(&canonical_ref)
              .map(|meta| oxc_ecmascript::constant_evaluation::ConstantValue::from(&meta.value))
          }),
          constant_map: &constant_map,
        };
        let mut ctx = Context::new(
          eval_ctx,
          module.default_export_ref,
          module_idx,
          &cross_module_inline_const_ctx.config,
          &self.side_effects_free_function_symbol_ref,
          &ContextOptions {
            symbols: &self.symbols,
            flat_options: self.flat_options,
            options: self.options,
            ast_scope: &self.symbols.local_db(module_idx).ast_scopes,
          },
        );
        ctx.visit_program(&dep.program);
        ctx.side_effect_detail_mutations.into_iter().for_each(|(stmt_idx, detail)| {
          side_effect_mutation_map.entry(module_idx).or_default().push((stmt_idx, detail));
        });
        if !ctx.local_symbol_map.is_empty() {
          cross_module_inline_const_ctx.changed = true;
          constant_symbol_map.extend(ctx.local_symbol_map);
        }
      });
    }
    for (module_idx, mutations) in side_effect_mutation_map {
      if let Some(module) = self.module_table[module_idx].as_normal_mut() {
        for (stmt_info_idx, side_effect_detail) in mutations {
          module.stmt_infos[stmt_info_idx].side_effect = side_effect_detail;
        }
      }
    }
  }
}

struct ContextOptions<'a> {
  symbols: &'a SymbolRefDb,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  ast_scope: &'a AstScopes,
}

struct Context<'a, 'ast: 'a> {
  local_symbol_map: FxHashMap<SymbolRef, ConstExportMeta>,
  side_effect_detail_mutations: FxHashMap<StmtInfoIdx, SideEffectDetail>,
  eval_ctx: ConstEvalCtx<'a, 'ast>,
  export_default_symbol: SymbolRef,
  module_idx: ModuleIdx,
  config: &'a CrossModuleOptimizationConfig,
  scope_stack: Vec<ScopeFlags>,
  traverse_state: TraverseState,
  side_effect_free_call_expr_addr: FxHashSet<Address>,
  global_side_effect_free_function_symbols_: &'a FxHashSet<SymbolRef>,
  symbols: &'a SymbolRefDb,
  flat_options: FlatOptions,
  options: &'a SharedNormalizedBundlerOptions,
  ast_scope: &'a AstScopes,
}

impl<'a, 'ast: 'a> Context<'a, 'ast> {
  fn new(
    eval_ctx: ConstEvalCtx<'a, 'ast>,
    export_default_symbol: SymbolRef,
    module_idx: ModuleIdx,
    config: &'a CrossModuleOptimizationConfig,
    global_side_effects_free_function_symbol_ref: &'a FxHashSet<SymbolRef>,
    ctx_options: &ContextOptions<'a>,
  ) -> Self {
    Self {
      local_symbol_map: FxHashMap::default(),
      eval_ctx,
      export_default_symbol,
      module_idx,
      config,
      scope_stack: vec![],
      traverse_state: TraverseState::empty(),
      side_effect_free_call_expr_addr: FxHashSet::default(),
      global_side_effect_free_function_symbols_: global_side_effects_free_function_symbol_ref,
      symbols: ctx_options.symbols,
      flat_options: ctx_options.flat_options,
      options: ctx_options.options,
      ast_scope: ctx_options.ast_scope,
      side_effect_detail_mutations: FxHashMap::default(),
    }
  }
}

impl<'a, 'ast: 'a> Visit<'ast> for Context<'a, 'ast> {
  fn enter_scope(
    &mut self,
    flags: oxc::semantic::ScopeFlags,
    _scope_id: &std::cell::Cell<Option<oxc::semantic::ScopeId>>,
  ) {
    self.scope_stack.push(flags);
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn leave_scope(&mut self) {
    self.scope_stack.pop();
    self.traverse_state.set(TraverseState::TopLevel, is_top_level(&self.scope_stack));
  }

  fn visit_program(&mut self, program: &oxc::ast::ast::Program<'ast>) {
    self.enter_scope(
      {
        let mut flags = ScopeFlags::Top;
        if program.source_type.is_strict() || program.has_use_strict_directive() {
          flags |= ScopeFlags::StrictMode;
        }
        flags
      },
      &program.scope_id,
    );
    // Custom visit
    for (idx, stmt) in program.body.iter().enumerate() {
      let pre_addr_len = self.side_effect_free_call_expr_addr.len();
      self.visit_statement(stmt);
      if pre_addr_len != self.side_effect_free_call_expr_addr.len() {
        let stmt_info_idx = StmtInfoIdx::new(idx + 1);
        let side_effect_detail = SideEffectDetector::new(
          self.ast_scope,
          self.flat_options,
          self.options,
          Some(&self.side_effect_free_call_expr_addr),
        )
        .detect_side_effect_of_stmt(stmt);
        self.side_effect_detail_mutations.insert(stmt_info_idx, side_effect_detail);
      }
    }

    self.leave_scope();
  }

  fn visit_call_expression(&mut self, it: &oxc::ast::ast::CallExpression<'ast>) {
    if self.traverse_state.contains(TraverseState::TopLevel) {
      let is_side_effects_free_function = it
        .callee
        .as_identifier()
        .and_then(|item| {
          item
            .reference_id
            .get()
            .and_then(|ref_id| self.eval_ctx.scope.get_reference(ref_id).symbol_id())
            .map(|id| {
              let symbol_ref = self.symbols.canonical_ref_for((self.module_idx, id).into());
              self.global_side_effect_free_function_symbols_.contains(&symbol_ref)
            })
        })
        .unwrap_or(false);

      if is_side_effects_free_function {
        self.side_effect_free_call_expr_addr.insert(Address::from_ptr(it));
      }
    }
    walk::walk_call_expression(self, it);
  }

  fn visit_export_named_declaration(&mut self, it: &ExportNamedDeclaration<'ast>) {
    if it.source.is_none()
      && self.config.inline_const_optimization
      && let Some(ref decl) = it.declaration
      && let Declaration::VariableDeclaration(var_decl) = decl
    {
      var_decl.declarations.iter().for_each(|declarator| {
        if let BindingPatternKind::BindingIdentifier(ref binding) = declarator.id.kind
          && let Some(value) = declarator
            .init
            .as_ref()
            .and_then(|expr| try_extract_const_literal(&self.eval_ctx, expr))
        {
          let symbol_ref: SymbolRef = (self.module_idx, binding.symbol_id()).into();

          if self.local_symbol_map.get(&symbol_ref).map(|meta| &meta.value) != Some(&value) {
            self.local_symbol_map.insert(symbol_ref, ConstExportMeta::new(value, false));
          }
        }
      });
    }
    walk::walk_export_named_declaration(self, it);
  }

  fn visit_export_default_declaration(&mut self, it: &ExportDefaultDeclaration<'ast>) {
    if !self.config.inline_const_optimization {
      return;
    }
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
    let symbol_ref: SymbolRef = (self.module_idx, symbol_id).into();
    if let Some(v) = try_extract_const_literal(&self.eval_ctx, expr) {
      if self.local_symbol_map.get(&symbol_ref).map(|meta| &meta.value) != Some(&v) {
        self.local_symbol_map.insert(symbol_ref, ConstExportMeta::new(v, false));
      }
    }
  }
}
