use oxc::ast::ast::{self, Declaration, ExportDefaultDeclarationKind, Statement};
use rolldown_common::{Module, SymbolOrMemberExprRef, SymbolRef, TaggedSymbolRef};
use rustc_hash::FxHashSet;

use super::LinkStage;

fn lexical_declaration(
  declaration: &Declaration<'_>,
  bindings: &mut Vec<(oxc::semantic::SymbolId, bool)>,
) {
  match declaration {
    Declaration::VariableDeclaration(declaration) if declaration.kind.is_lexical() => {
      for declarator in &declaration.declarations {
        for binding in declarator.id.get_binding_identifiers() {
          bindings.push((binding.symbol_id(), declaration.kind.is_const()));
        }
      }
    }
    Declaration::ClassDeclaration(class) => {
      if let Some(id) = &class.id {
        bindings.push((id.symbol_id(), false));
      }
    }
    _ => {}
  }
}

fn is_primitive_literal(expr: &ast::Expression<'_>) -> bool {
  matches!(
    expr,
    ast::Expression::BooleanLiteral(_)
      | ast::Expression::NumericLiteral(_)
      | ast::Expression::StringLiteral(_)
      | ast::Expression::NullLiteral(_)
      | ast::Expression::BigIntLiteral(_)
  )
}

fn is_safe_define_property_call(expr: &ast::Expression<'_>) -> bool {
  let ast::Expression::CallExpression(call) = expr else {
    return false;
  };
  let ast::Expression::StaticMemberExpression(member) = &call.callee else {
    return false;
  };
  if !matches!(
    member.property.name.as_str(),
    "defineProperty" | "defineProperties"
  ) {
    return false;
  }
  if !matches!(&member.object, ast::Expression::Identifier(ident) if ident.name == "Object") {
    return false;
  }
  if call.arguments.len() < 2 {
    return false;
  }
  let Some(target) = call.arguments.first().and_then(|arg| arg.as_expression()) else {
    return false;
  };
  let target_ok =
    matches!(target, ast::Expression::ObjectExpression(obj) if obj.properties.is_empty())
      || is_safe_define_property_call(target);
  target_ok
    && call.arguments[1..].iter().all(|arg| {
      let Some(arg_expr) = arg.as_expression() else {
        return false;
      };
      if is_primitive_literal(arg_expr) {
        return true;
      }
      if let ast::Expression::ObjectExpression(obj) = arg_expr {
        return obj.properties.iter().all(|prop| match prop {
          ast::ObjectPropertyKind::ObjectProperty(p) => {
            !p.computed
              && matches!(p.kind, ast::PropertyKind::Init)
              && (is_primitive_literal(&p.value)
                || matches!(
                  &p.value,
                  ast::Expression::FunctionExpression(_)
                    | ast::Expression::ArrowFunctionExpression(_)
                ))
          }
          ast::ObjectPropertyKind::SpreadProperty(_) => false,
        });
      }
      false
    })
}

fn is_safe_compound_const_init(expr: &ast::Expression<'_>, depth: usize) -> bool {
  if depth > 6 {
    return false;
  }
  if is_primitive_literal(expr) || is_safe_define_property_call(expr) {
    return true;
  }
  match expr {
    ast::Expression::ObjectExpression(obj) => obj.properties.iter().all(|prop| match prop {
      ast::ObjectPropertyKind::ObjectProperty(p) => {
        !p.computed
          && matches!(p.kind, ast::PropertyKind::Init)
          && is_safe_compound_const_init(&p.value, depth + 1)
      }
      ast::ObjectPropertyKind::SpreadProperty(_) => false,
    }),
    ast::Expression::ArrayExpression(arr) => arr.elements.iter().all(|elem| match elem {
      ast::ArrayExpressionElement::SpreadElement(_) | ast::ArrayExpressionElement::Elision(_) => {
        false
      }
      _ => elem
        .as_expression()
        .is_some_and(|e| is_safe_compound_const_init(e, depth + 1)),
    }),
    ast::Expression::CallExpression(call) => {
      if call.arguments.len() != 1 {
        return false;
      }
      let Some(arg) = call.arguments[0].as_expression() else {
        return false;
      };
      let ast::Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
      };
      if matches!(
        member.property.name.as_str(),
        "freeze" | "seal" | "preventExtensions"
      ) && matches!(&member.object, ast::Expression::Identifier(ident) if ident.name == "Object")
      {
        return is_safe_compound_const_init(arg, depth + 1);
      }
      if member.property.name == "split"
        && matches!(&member.object, ast::Expression::StringLiteral(_))
        && matches!(arg, ast::Expression::StringLiteral(_))
      {
        return true;
      }
      false
    }
    _ => false,
  }
}

fn literal_const_prefix(
  program: &ast::Program<'_>,
  allow_minified_compounds: bool,
) -> (FxHashSet<oxc::semantic::SymbolId>, FxHashSet<oxc::semantic::SymbolId>) {
  let mut primitive_bindings = FxHashSet::default();
  let mut compound_bindings = FxHashSet::default();
  'prefix: for statement in &program.body {
    let declaration = match statement {
      Statement::VariableDeclaration(declaration) => declaration,
      Statement::ExportDeclaration(export) => match &export.declaration {
        Declaration::VariableDeclaration(declaration) => declaration,
        Declaration::FunctionDeclaration(_) => continue,
        _ => break,
      },
      Statement::FunctionDeclaration(_)
      | Statement::ExportNamedDeclaration(_)
      | Statement::EmptyStatement(_) => continue,
      Statement::ImportDeclaration(_) if allow_minified_compounds => continue,
      Statement::ExportDefaultDeclaration(export)
        if matches!(export.declaration, ExportDefaultDeclarationKind::FunctionDeclaration(_)) =>
      {
        continue;
      }
      _ => break,
    };
    if !declaration.kind.is_const() {
      break;
    }
    for declarator in &declaration.declarations {
      let ast::BindingPattern::BindingIdentifier(binding) = &declarator.id else {
        break 'prefix;
      };
      let Some(init) = declarator.init.as_ref() else {
        break 'prefix;
      };
      if is_primitive_literal(init) {
        primitive_bindings.insert(binding.symbol_id());
      } else if is_safe_define_property_call(init)
        || (allow_minified_compounds && is_safe_compound_const_init(init, 0))
      {
        compound_bindings.insert(binding.symbol_id());
      } else {
        break 'prefix;
      }
    }
  }
  (primitive_bindings, compound_bindings)
}

impl LinkStage<'_> {
  pub(super) fn reference_async_evaluation_helpers(&mut self) {
    let start = self.runtime.resolve_symbol("__startAsync");
    let finish = self.runtime.resolve_symbol("__finishAsync");
    let initialize = self.runtime.resolve_symbol("__esmAsync");
    for (idx, infos) in self.stmt_infos.iter_mut_enumerated() {
      if self.metas[idx].is_tla_or_contains_tla_dependency {
        for info in &mut infos.infos {
          info.referenced_symbols.extend([start.into(), finish.into(), initialize.into()]);
        }
      }
    }
  }

  pub(super) fn preserve_temporal_bindings(&mut self) {
    let native_records = crate::utils::native_records::enabled(self.options);
    // An acyclic leaf alone is insufficient: a cyclic barrel can read its reexport before
    // traversal reaches the leaf. Every static edge must respect the completed DFS order.
    let acyclic_full_graph = !self.options.is_dev_mode_enabled()
      && self.module_table.iter().all(|module| {
        module.as_normal().is_none_or(|module| !module.meta.has_eval())
          && module.import_records().iter().filter(|record| record.kind.is_static()).all(|record| {
            record.resolved_module.is_none_or(|dependency| {
              self.module_table[dependency].exec_order() < module.exec_order()
            })
          })
      });
    for module in self.module_table.modules.iter().filter_map(Module::as_normal) {
      if module.idx == self.runtime.id() || module.exports_kind.is_commonjs() {
        continue;
      }
      let mut bindings = Vec::new();
      let scopes =
        self.symbols[module.idx].as_ref().expect("normal module symbols").ast_scopes.scoping();
      for symbol in scopes.symbol_ids() {
        let flags = scopes.symbol_flags(symbol);
        if scopes.symbol_scope_id(symbol) == scopes.root_scope_id()
          && flags.intersects(
            oxc::syntax::symbol::SymbolFlags::BlockScopedVariable
              | oxc::syntax::symbol::SymbolFlags::Class,
          )
        {
          bindings.push((symbol, flags.is_const_variable()));
        }
      }
      let scope_bindings = bindings.len();
      let ast = self.ast_table[module.idx].as_ref().expect("normal module AST");
      let allow_minified_compounds = self.options.minify.is_enabled()
        && scopes
          .get_binding(scopes.root_scope_id(), "Object".into())
          .is_none();
      let object_unshadowed = scopes
        .get_binding(scopes.root_scope_id(), "Object".into())
        .is_none();
      let safe_literals = if acyclic_full_graph
        && !module.meta.has_eval()
        && !self.metas[module.idx].is_tla_or_contains_tla_dependency
        && (allow_minified_compounds
          || !module.import_records.iter().any(|record| record.kind.is_static()))
      {
        let (primitives, compounds) =
          literal_const_prefix(ast.program(), allow_minified_compounds);
        let mut set: FxHashSet<_> = primitives
          .into_iter()
          // The scanner excludes mutated bindings from its constant map.
          .filter(|symbol| {
            self.global_constant_symbol_map.contains_key(&(module.idx, *symbol).into())
          })
          .collect();
        if object_unshadowed {
          for symbol in compounds {
            if !scopes.get_resolved_references(symbol).any(|r| r.is_write()) {
              set.insert(symbol);
            }
          }
        }
        set
      } else {
        FxHashSet::default()
      };
      for statement in &ast.program().body {
        let variables = match statement {
          Statement::ExportDeclaration(export) => match &export.declaration {
            Declaration::VariableDeclaration(declaration) => Some(declaration),
            _ => None,
          },
          Statement::VariableDeclaration(declaration) => Some(declaration),
          _ => None,
        };
        if let Some(declaration) = variables {
          for declarator in &declaration.declarations {
            for binding in declarator.id.get_binding_identifiers() {
              // Even var exports contain undefined before their initializer executes.
              if !safe_literals.contains(&binding.symbol_id()) {
                self.global_constant_symbol_map.remove(&(module.idx, binding.symbol_id()).into());
              }
            }
          }
        }
        match statement {
          Statement::ExportDeclaration(export) => {
            lexical_declaration(&export.declaration, &mut bindings);
          }
          Statement::VariableDeclaration(declaration) if declaration.kind.is_lexical() => {
            for declarator in &declaration.declarations {
              for binding in declarator.id.get_binding_identifiers() {
                bindings.push((binding.symbol_id(), declaration.kind.is_const()));
              }
            }
          }
          Statement::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
              bindings.push((id.symbol_id(), false));
            }
          }
          Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(_) => {}
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
              bindings.push((
                class.id.as_ref().map_or(module.default_export_ref.symbol, |id| id.symbol_id()),
                false,
              ));
            }
            ast::match_expression!(ExportDefaultDeclarationKind) => {
              bindings.push((module.default_export_ref.symbol, true));
              if native_records {
                self.metas[module.idx].native_default_expression = true;
              }
            }
            _ => {}
          },
          _ => {}
        }
      }
      // Bindings found only through scope flags were lowered by a transform (for example
      // `await using` turns the remaining top-level declarations into `var`s inside a `try`).
      let authored_lexical: FxHashSet<_> =
        bindings[scope_bindings..].iter().map(|(symbol, _)| *symbol).collect();
      bindings.sort_unstable_by_key(|(symbol, _)| *symbol);
      bindings.dedup_by_key(|(symbol, _)| *symbol);
      for (symbol, constant) in bindings {
        if native_records && authored_lexical.contains(&symbol) {
          // Each preserved development module is its own native module record: the engine
          // already enforces TDZ and const assignment for real lexical declarations.
          self.global_constant_symbol_map.remove(&(module.idx, symbol).into());
          continue;
        }
        if safe_literals.contains(&symbol) {
          continue;
        }
        let reference: SymbolRef = (module.idx, symbol).into();
        let flag = self.symbols.create_facade_root_symbol_ref(module.idx, "__initialized");
        self.metas[module.idx].temporal_symbols.insert(reference, flag);
        if constant {
          self.metas[module.idx].temporal_constants.insert(reference);
        }
        self.global_constant_symbol_map.remove(&reference);
        let declarations =
          self.stmt_infos[module.idx].declared_stmts_by_symbol(&reference).to_vec();
        for statement in declarations {
          self.stmt_infos[module.idx]
            .declare_symbol_for_stmt(statement, TaggedSymbolRef::normal(flag));
          self.stmt_infos[module.idx][statement].referenced_symbols.push(flag.into());
        }
      }
    }

    let any_temporal_symbols = self.metas.iter().any(|meta| !meta.temporal_symbols.is_empty());
    if !any_temporal_symbols {
      return;
    }
    let check = self.runtime.resolve_symbol("__checkTDZ");
    for (idx, infos) in self.stmt_infos.iter_mut_enumerated() {
      if idx == self.runtime.id() {
        continue;
      }
      for info in &mut infos.infos {
        let mut flags = Vec::new();
        for reference in &info.referenced_symbols {
          let resolved = match reference {
            SymbolOrMemberExprRef::MemberExpr(member) => Some(
              member
                .resolution(&self.metas[idx].resolved_member_expr_refs)
                .and_then(|resolution| resolution.resolved)
                .unwrap_or(member.object_ref),
            ),
            SymbolOrMemberExprRef::Symbol(symbol) => Some(*symbol),
          };
          if let Some(resolved) = resolved {
            let canonical = self.symbols.canonical_ref_for(resolved);
            if let Some(flag) = self.metas[canonical.owner].temporal_symbols.get(&canonical) {
              // Another record's flag would have to become one of its exports, an extra key
              // in its native namespace. Its namespace reaches the flag through the runtime.
              let native_owner = (canonical.owner != idx)
                .then(|| self.module_table[canonical.owner].as_normal())
                .flatten()
                .filter(|owner| {
                  crate::utils::native_records::has_native_namespace(
                    self.options,
                    owner,
                    &self.metas[owner.idx],
                    &self.symbols,
                  )
                });
              flags.push(native_owner.map_or(*flag, |owner| owner.namespace_object_ref));
            }
          }
        }
        if !flags.is_empty() {
          flags.sort_unstable();
          flags.dedup();
          info.referenced_symbols.extend(flags.into_iter().map(Into::into));
          info.referenced_symbols.push(check.into());
        }
      }
    }
  }
}
