use oxc::{
  ast::{
    AstKind,
    ast::{
      self, BindingPattern, Declaration, ExportDefaultDeclarationKind, Expression,
      IdentifierReference, Program, Statement, VariableDeclarationKind,
    },
  },
  ast_visit::VisitJs,
  semantic::{ReferenceId, SymbolId},
};
use oxc_ecmascript::{
  GlobalContext,
  side_effects::{MayHaveSideEffectsContext, PropertyReadSideEffects},
};
use oxc_index::IndexVec;
use rolldown_common::{
  AstScopes, EcmaViewMeta, ExportsKind, FlatOptions, IndexModules, ModuleIdx, ModuleType,
  NormalModule, NormalizedBundlerOptions, PropertyWriteSideEffects, StmtInfoIdx, SymbolRef,
  SymbolRefDb, WrapKind, side_effects::DeterminedSideEffects,
};
use rustc_hash::FxHashMap;

use crate::{
  stages::link_stage::refine_stmt_side_effects::ImportCycles, type_alias::IndexStmtInfos,
  types::linking_metadata::LinkingMetadataVec,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclKind {
  Var,
  Let,
  Const,
  Function,
  Class,
  /// `export default <expression>`: a hidden `const`.
  Default,
  /// A binding that the analysis does not track, such as one inside a destructuring pattern.
  Other,
}

#[derive(Debug, Clone, Copy)]
pub enum Init<'a> {
  None,
  Expr(&'a Expression<'a>),
  Function(&'a ast::Function<'a>),
}

/// The top-level declaration of a binding.
#[derive(Debug, Clone, Copy)]
pub struct Decl<'a> {
  pub stmt: StmtInfoIdx,
  pub kind: DeclKind,
  pub init: Init<'a>,
  /// The binding has exactly one declaration.
  pub unique: bool,
}

/// Per-module facts that do not change during the analysis.
pub struct ModuleFacts<'a> {
  pub idx: ModuleIdx,
  pub module: &'a NormalModule,
  pub program: &'a Program<'a>,
  pub scopes: &'a AstScopes,
  pub decls: FxHashMap<SymbolId, Decl<'a>>,
  /// The function that a hoisted top-level function declaration statement declares.
  pub hoisted_fns: FxHashMap<StmtInfoIdx, SymbolId>,
  /// For each top-level function, the first top-level statement that can call it. The
  /// statement refers to the function, or to a function that refers to it. A function that no
  /// statement reaches runs only after the module finished, if the module is not in a cycle.
  pub fn_first_reached: FxHashMap<SymbolId, StmtInfoIdx>,
  /// The module is in a static-import cycle, which includes a cycle with itself.
  pub in_cycle: bool,
}

impl<'a> ModuleFacts<'a> {
  /// The top-level statement for a statement-info index, if the index maps to the AST.
  pub fn stmt(&self, stmt_idx: StmtInfoIdx) -> Option<&'a Statement<'a>> {
    stmt_idx.index().checked_sub(1).and_then(|idx| self.program.body.get(idx))
  }

  pub fn is_named_import(&self, symbol_id: SymbolId) -> bool {
    self.module.named_imports.contains_key(&SymbolRef::from((self.idx, symbol_id)))
  }
}

/// The inputs of the analysis. None of them change during the analysis.
pub struct Env<'a> {
  pub modules: &'a IndexModules,
  pub symbols: &'a SymbolRefDb,
  pub stmt_infos: &'a IndexStmtInfos,
  pub metas: &'a LinkingMetadataVec,
  pub options: &'a NormalizedBundlerOptions,
  pub flat_options: FlatOptions,
  pub(in crate::stages::link_stage) import_cycles: &'a ImportCycles,
  /// `None` for modules that the analysis does not cover.
  pub facts: IndexVec<ModuleIdx, Option<ModuleFacts<'a>>>,
}

impl<'a> Env<'a> {
  #[expect(clippy::too_many_arguments)]
  pub(in crate::stages::link_stage) fn new(
    modules: &'a IndexModules,
    ast_table: &'a crate::type_alias::IndexEcmaAst,
    symbols: &'a SymbolRefDb,
    stmt_infos: &'a IndexStmtInfos,
    metas: &'a LinkingMetadataVec,
    options: &'a NormalizedBundlerOptions,
    flat_options: FlatOptions,
    import_cycles: &'a ImportCycles,
    runtime_idx: ModuleIdx,
  ) -> Self {
    // With `propertyWriteSideEffects: false`, the sweep never checks a property write, so the
    // analysis would miss its deopt.
    let enabled =
      !matches!(options.treeshake.property_write_side_effects(), PropertyWriteSideEffects::False);
    let facts = modules
      .iter()
      .map(|module| {
        let module = module.as_normal()?;
        let meta = &metas[module.idx];
        let analyzable = enabled
          && module.idx != runtime_idx
          && matches!(module.exports_kind, ExportsKind::Esm | ExportsKind::None)
          && matches!(meta.wrap_kind(), WrapKind::None)
          && matches!(
            module.module_type,
            ModuleType::Js | ModuleType::Jsx | ModuleType::Ts | ModuleType::Tsx
          )
          && !module.meta.has_eval()
          && !module.meta.contains(EcmaViewMeta::HasLazyExport)
          && !meta.is_tla_or_contains_tla_dependency
          && !matches!(module.side_effects, DeterminedSideEffects::NoTreeshake);
        if !analyzable {
          return None;
        }
        let program = ast_table[module.idx].as_ref()?.program();
        let scopes = &symbols[module.idx].as_ref()?.ast_scopes;
        let in_cycle = import_cycles.is_in_cycle(modules, module.idx)
          || module
            .import_records
            .iter()
            .any(|record| record.kind.is_static() && record.resolved_module == Some(module.idx));
        Some(build_module_facts(module, program, scopes, in_cycle))
      })
      .collect();
    Self { modules, symbols, stmt_infos, metas, options, flat_options, import_cycles, facts }
  }

  pub fn module(&self, idx: ModuleIdx) -> Option<&ModuleFacts<'a>> {
    self.facts.get(idx).and_then(Option::as_ref)
  }

  pub fn decl(&self, symbol_ref: SymbolRef) -> Option<Decl<'a>> {
    self.module(symbol_ref.owner)?.decls.get(&symbol_ref.symbol).copied()
  }

  pub fn canonical(&self, module: ModuleIdx, symbol_id: SymbolId) -> SymbolRef {
    self.symbols.canonical_ref_for((module, symbol_id).into())
  }

  pub fn annotations(&self) -> bool {
    !self.flat_options.ignore_annotations()
  }

  /// The analyzed module whose namespace object `canonical` is.
  pub fn namespace_module(&self, canonical: SymbolRef) -> Option<ModuleIdx> {
    let facts = self.module(canonical.owner)?;
    (facts.module.namespace_object_ref == canonical).then_some(canonical.owner)
  }

  /// The canonical binding of the export `key` of `module`, if the namespace object has it.
  pub fn namespace_export(&self, module: ModuleIdx, key: &str) -> Option<SymbolRef> {
    let meta = &self.metas[module];
    match meta.sorted_and_non_ambiguous_resolved_exports.get(key) {
      Some(false) => {
        Some(self.symbols.canonical_ref_for(meta.resolved_exports.get(key)?.symbol_ref))
      }
      _ => None,
    }
  }

  /// Whether the namespace object of `module` has the export `key`. `None` when the answer
  /// depends on code that the analysis cannot see.
  pub fn namespace_has(&self, module: ModuleIdx, key: &str) -> Option<bool> {
    let meta = &self.metas[module];
    match meta.sorted_and_non_ambiguous_resolved_exports.get(key) {
      Some(false) => Some(true),
      Some(true) => None,
      None => {
        let unknown = key == "__esModule"
          || meta.has_dynamic_exports
          || meta.resolved_exports.contains_key(key)
          || !meta.shimmed_missing_exports.is_empty()
          || self.options.shim_missing_exports;
        (!unknown).then_some(false)
      }
    }
  }

  /// Whether a read of `binding` at `site` cannot see it uninitialized, so its value is the
  /// value of its initializer.
  pub fn is_init_safe(&self, site: Site, binding: SymbolRef, decl: &Decl<'a>) -> bool {
    if decl.kind == DeclKind::Function {
      // Function declarations initialize when the module instantiates.
      return true;
    }
    let Some(owner) = self.module(binding.owner) else {
      return false;
    };
    if site.module == binding.owner {
      if !site.in_function {
        return site.stmt > decl.stmt;
      }
      // A function created by a statement exists only after that statement ran. A hoisted
      // function exists from the start, so only a statement that reaches it, or a module in
      // the same cycle, can call it before the declaration ran.
      return match owner.hoisted_fns.get(&site.stmt) {
        Some(function) => {
          !owner.in_cycle
            && owner.fn_first_reached.get(function).is_none_or(|first| *first > decl.stmt)
        }
        None => site.stmt > decl.stmt,
      };
    }
    // Another module reads it. The owner must finish before the reader starts. Then a function
    // of the reader can run early only if the reader is in a cycle.
    let Some(reader) = self.module(site.module) else {
      return false;
    };
    owner.module.exec_order < reader.module.exec_order
      && !self.import_cycles.same_cycle(self.modules, site.module, binding.owner)
      && (!site.in_function || !reader.in_cycle)
  }

  /// Whether every binding of `owner`, a module that the analysis does not cover, is
  /// initialized when code at `site` runs. The same rule as for another analyzed module.
  pub fn is_foreign_init_safe(&self, site: Site, owner: ModuleIdx) -> bool {
    let Some(owner_module) = self.modules[owner].as_normal() else {
      // An external module runs before the modules that import it.
      return true;
    };
    let Some(reader) = self.module(site.module) else {
      return false;
    };
    owner_module.exec_order < reader.module.exec_order
      && !self.import_cycles.same_cycle(self.modules, site.module, owner)
      && (!site.in_function || !reader.in_cycle)
  }
}

/// Where code runs: the top-level statement that contains it, and whether it is inside a
/// function body.
#[derive(Debug, Clone, Copy)]
pub struct Site {
  pub module: ModuleIdx,
  pub stmt: StmtInfoIdx,
  pub in_function: bool,
}

impl Site {
  pub fn in_function(self) -> Self {
    Self { in_function: true, ..self }
  }
}

fn build_module_facts<'a>(
  module: &'a NormalModule,
  program: &'a Program<'a>,
  scopes: &'a AstScopes,
  in_cycle: bool,
) -> ModuleFacts<'a> {
  let mut decls = FxHashMap::<SymbolId, Decl<'a>>::default();
  let mut hoisted_fns = FxHashMap::<StmtInfoIdx, SymbolId>::default();
  let scoping = scopes.scoping();
  let mut add = |symbol_id: Option<SymbolId>, stmt: StmtInfoIdx, kind: DeclKind, init: Init<'a>| {
    let Some(symbol_id) = symbol_id else {
      return;
    };
    let unique = scoping.symbol_redeclarations(symbol_id).is_empty();
    decls.entry(symbol_id).and_modify(|decl| decl.unique = false).or_insert(Decl {
      stmt,
      kind,
      init,
      unique,
    });
  };
  for (idx, stmt) in program.body.iter().enumerate() {
    let stmt_idx = StmtInfoIdx::from_usize(idx + 1);
    let decl = match stmt {
      Statement::ExportDeclaration(export) => Some(&export.declaration),
      match_declaration_stmt @ oxc::ast::match_declaration!(Statement) => {
        Some(match_declaration_stmt.to_declaration())
      }
      Statement::ExportDefaultDeclaration(export) => {
        match &export.declaration {
          ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            let symbol_id = func
              .id
              .as_ref()
              .and_then(|id| id.symbol_id.get())
              .unwrap_or(module.default_export_ref.symbol);
            add(Some(symbol_id), stmt_idx, DeclKind::Function, Init::Function(func));
            hoisted_fns.insert(stmt_idx, symbol_id);
          }
          ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            let symbol_id = class
              .id
              .as_ref()
              .and_then(|id| id.symbol_id.get())
              .unwrap_or(module.default_export_ref.symbol);
            add(Some(symbol_id), stmt_idx, DeclKind::Class, Init::None);
          }
          ExportDefaultDeclarationKind::TSInterfaceDeclaration(_) => {}
          expr => add(
            Some(module.default_export_ref.symbol),
            stmt_idx,
            DeclKind::Default,
            Init::Expr(expr.to_expression()),
          ),
        }
        None
      }
      _ => None,
    };
    let Some(decl) = decl else {
      continue;
    };
    match decl {
      Declaration::VariableDeclaration(var_decl) => {
        let kind = match var_decl.kind {
          VariableDeclarationKind::Var => DeclKind::Var,
          VariableDeclarationKind::Let => DeclKind::Let,
          VariableDeclarationKind::Const => DeclKind::Const,
          VariableDeclarationKind::Using | VariableDeclarationKind::AwaitUsing => DeclKind::Other,
        };
        for declarator in &var_decl.declarations {
          match &declarator.id {
            BindingPattern::BindingIdentifier(id) => add(
              id.symbol_id.get(),
              stmt_idx,
              kind,
              declarator.init.as_ref().map_or(Init::None, Init::Expr),
            ),
            pattern => {
              for id in pattern.get_binding_identifiers() {
                add(id.symbol_id.get(), stmt_idx, DeclKind::Other, Init::None);
              }
            }
          }
        }
      }
      Declaration::FunctionDeclaration(func) => {
        if let Some(symbol_id) = func.id.as_ref().and_then(|id| id.symbol_id.get()) {
          add(Some(symbol_id), stmt_idx, DeclKind::Function, Init::Function(func));
          hoisted_fns.insert(stmt_idx, symbol_id);
        }
      }
      Declaration::ClassDeclaration(class) => {
        if let Some(id) = &class.id {
          add(id.symbol_id.get(), stmt_idx, DeclKind::Class, Init::None);
        }
      }
      _ => {}
    }
  }
  let fn_first_reached = compute_fn_first_reached(program, scopes, &decls, &hoisted_fns);
  ModuleFacts {
    idx: module.idx,
    module,
    program,
    scopes,
    decls,
    hoisted_fns,
    fn_first_reached,
    in_cycle,
  }
}

/// For each top-level function, the first statement that refers to it, directly or through the
/// bodies of other top-level functions. Hoisted function declarations do not run, so they reach
/// nothing by themselves. A statement that only lists exports calls nothing.
fn compute_fn_first_reached<'a>(
  program: &'a Program<'a>,
  scopes: &AstScopes,
  decls: &FxHashMap<SymbolId, Decl<'a>>,
  hoisted_fns: &FxHashMap<StmtInfoIdx, SymbolId>,
) -> FxHashMap<SymbolId, StmtInfoIdx> {
  struct FnRefs<'s, 'a> {
    scopes: &'s AstScopes,
    functions: &'s FxHashMap<SymbolId, &'a ast::Function<'a>>,
    found: Vec<SymbolId>,
  }

  impl<'v> VisitJs<'v> for FnRefs<'_, '_> {
    fn enter_node(&mut self, kind: AstKind<'v>) {
      if let AstKind::IdentifierReference(ident) = kind
        && let Some(symbol_id) =
          ident.reference_id.get().and_then(|ref_id| self.scopes.symbol_id_for(ref_id))
        && self.functions.contains_key(&symbol_id)
      {
        self.found.push(symbol_id);
      }
    }
  }

  let functions: FxHashMap<SymbolId, &ast::Function> = decls
    .iter()
    .filter_map(|(symbol_id, decl)| match decl.init {
      Init::Function(func) if decl.kind == DeclKind::Function => Some((*symbol_id, func)),
      _ => None,
    })
    .collect();
  let mut first_reached = FxHashMap::default();
  if functions.is_empty() {
    return first_reached;
  }
  let mut finder = FnRefs { scopes, functions: &functions, found: Vec::new() };
  for (idx, stmt) in program.body.iter().enumerate() {
    let stmt_idx = StmtInfoIdx::from_usize(idx + 1);
    let lists_exports_only = matches!(
      stmt,
      Statement::ImportDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::ExportNamedDeclaration(_)
        | Statement::ExportFromDeclaration(_)
    );
    if hoisted_fns.contains_key(&stmt_idx) || lists_exports_only {
      continue;
    }
    finder.visit_statement(stmt);
    while let Some(symbol_id) = finder.found.pop() {
      if first_reached.contains_key(&symbol_id) {
        continue;
      }
      first_reached.insert(symbol_id, stmt_idx);
      if let Some(func) = functions.get(&symbol_id) {
        finder.visit_function(func, oxc::semantic::ScopeFlags::Function);
      }
    }
  }
  first_reached
}

/// The oxc side-effect context for fallback checks in one module. It knows nothing about the
/// facts of the analysis, so its answers are conservative.
pub struct OxcCtx<'s, 'a> {
  pub env: &'s Env<'a>,
  pub facts: &'s ModuleFacts<'a>,
}

impl<'ast> GlobalContext<'ast> for OxcCtx<'_, '_> {
  fn is_global_reference(&self, reference: &IdentifierReference<'ast>) -> bool {
    reference.reference_id.get().is_some_and(|ref_id| self.facts.scopes.is_unresolved(ref_id))
  }

  fn get_constant_value_for_reference_id(
    &self,
    _reference_id: ReferenceId,
  ) -> Option<oxc_ecmascript::ConstantValue<'ast>> {
    None
  }
}

impl MayHaveSideEffectsContext<'_> for OxcCtx<'_, '_> {
  fn engine_targets(&self) -> Option<&oxc::transformer::EngineTargets> {
    Some(&self.env.options.transform_options.target)
  }

  fn annotations(&self) -> bool {
    self.env.annotations()
  }

  fn manual_pure_functions(&self, callee: &Expression) -> bool {
    is_manual_pure(self.env, callee)
  }

  fn property_read_side_effects(&self) -> PropertyReadSideEffects {
    if self.env.flat_options.property_read_side_effects() {
      PropertyReadSideEffects::All
    } else {
      PropertyReadSideEffects::None
    }
  }

  fn unknown_global_side_effects(&self) -> bool {
    self.env.options.treeshake.unknown_global_side_effects()
  }

  fn property_write_side_effects(&self) -> bool {
    // The analysis decides writes itself. A write that oxc calls free would skip a deopt.
    true
  }
}

/// Whether `callee` matches `treeshake.manualPureFunctions`.
pub fn is_manual_pure(env: &Env, callee: &Expression) -> bool {
  let Some(names) = env.options.treeshake.manual_pure_functions() else {
    return false;
  };
  let mut expr = callee;
  loop {
    match expr.get_inner_expression() {
      Expression::Identifier(ident) => return names.contains(ident.name.as_str()),
      Expression::StaticMemberExpression(member) => expr = &member.object,
      Expression::ComputedMemberExpression(member) => expr = &member.object,
      Expression::CallExpression(call) => expr = &call.callee,
      _ => return false,
    }
  }
}
