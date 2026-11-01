use oxc::{
  ast::{
    AstKind,
    ast::{self, Argument, IdentifierReference},
  },
  ast_visit::VisitJs,
  semantic::{ScopeFlags, SymbolId},
};
use oxc_str::{CompactStr, JSStr};
use rolldown_common::{ImportRecordIdx, dynamic_import_usage::DynamicImportExportsUsage};
use rustc_hash::FxHashSet;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn update_dynamic_import_binding_usage_info(
    &mut self,
    ident: &IdentifierReference,
  ) -> Option<()> {
    // Each reference is counted once, as it is visited. One visited before its binding was
    // registered stays in the set: see `complete_dynamic_imports_read_before_registration`.
    if !self
      .dynamic_import_usage_info
      .dynamic_import_binding_reference_id
      .remove(&ident.reference_id())
    {
      return None;
    }

    let reference = self.result.symbol_ref_db.scoping().get_reference(ident.reference_id());

    // panic because if program reached here, means the BindingIdentifier has referenced the
    // IdentifierReference, but IdentifierReference did not saved the related `SymbolId`
    // Something wrong with semantic analyze
    let symbol_id = reference.symbol_id().expect("should have symbol id");
    let parent_idx = self.effective_parent_idx()?;
    let parent = self.visit_path[parent_idx];
    // if the property could be converted as a static property name, e.g.
    // a.b // static
    // a['b'] // static
    // a[b] // dynamic
    // Rolldown stores export names as UTF-8, so a name with a lone surrogate is treated as
    // dynamic.
    let partial_name = parent
      .as_member_expression_kind()
      .and_then(|expr| expr.static_property_name().and_then(JSStr::as_str));
    let rec_idx = *self
      .dynamic_import_usage_info
      .dynamic_import_binding_to_import_record_idx
      .get(&symbol_id)?;
    let usage = match partial_name {
      Some(name) => {
        if self.dynamic_import_member_is_called(parent_idx) {
          self.record_dynamic_import_called_name(rec_idx, name);
        }
        DynamicImportExportsUsage::Single(name.into())
      }
      None => match parent {
        // The binding is destructured in a later statement rather than accessed as a
        // member, e.g. `import('mod').then(ns => { const { a } = ns })`.
        // Treat the destructured keys the same as `ns.a` member accesses instead of bailing.
        AstKind::VariableDeclarator(var_decl)
          if matches!(&var_decl.id, ast::BindingPattern::ObjectPattern(_)) =>
        {
          // A re-exported key (`export const { a } = ns`) must be kept even when it
          // is never read locally, so detect the enclosing export declaration
          // (VariableDeclarator -> VariableDeclaration -> Export*) and forward it.
          let is_exported = parent_idx
            .checked_sub(2)
            .and_then(|idx| self.visit_path.get(idx))
            .is_some_and(|kind| {
              matches!(kind, AstKind::ExportDeclaration(_) | AstKind::ExportDefaultDeclaration(_))
            });
          match self.update_dynamic_import_usage_info_from_binding_pattern(
            &var_decl.id,
            rec_idx,
            is_exported,
          ) {
            Some(set) => DynamicImportExportsUsage::Partial(set),
            None => DynamicImportExportsUsage::Complete,
          }
        }
        _ => DynamicImportExportsUsage::Complete,
      },
    };
    match self.dynamic_import_usage_info.dynamic_import_exports_usage.entry(rec_idx) {
      std::collections::hash_map::Entry::Occupied(mut occ) => occ.get_mut().merge(usage),
      std::collections::hash_map::Entry::Vacant(vac) => {
        vac.insert(usage);
      }
    }

    None
  }

  /// Counts as completely used each `import()` whose binding has a reference that the scan visited
  /// before it registered the binding, such as a read in a closure declared above
  /// `const ns = await import('mod')`: the scan never saw how that reference reads the namespace.
  pub fn complete_dynamic_imports_read_before_registration(&mut self) {
    let scoping = self.result.symbol_ref_db.scoping();
    let info = &mut self.dynamic_import_usage_info;
    for reference_id in info.dynamic_import_binding_reference_id.drain() {
      let Some(symbol_id) = scoping.get_reference(reference_id).symbol_id() else {
        continue;
      };
      if let Some(&rec_idx) = info.dynamic_import_binding_to_import_record_idx.get(&symbol_id) {
        info.dynamic_import_exports_usage.insert(rec_idx, DynamicImportExportsUsage::Complete);
      }
    }
  }

  /// See internal-docs/code-splitting/implementation.md.
  fn record_dynamic_import_called_name(&mut self, rec_idx: ImportRecordIdx, name: &str) {
    self.result.dynamic_import_called_names.entry(rec_idx).or_default().insert(name.into());
  }

  fn effective_parent_idx(&self) -> Option<usize> {
    let mut idx = self.visit_path.len().checked_sub(1)?;
    if idx >= 1
      && let Some(AstKind::CallExpression(call)) = self.visit_path.get(idx)
      && matches!(
        &call.callee,
        ast::Expression::Identifier(ident) if ident.name == "\0native_temporal_read"
      )
    {
      idx -= 1;
    }
    Some(idx)
  }

  fn dynamic_import_member_is_called(&self, member_idx: usize) -> bool {
    let mut callee_id = self.visit_path[member_idx].node_id();
    for ancestor in self.visit_path[..member_idx].iter().rev() {
      match ancestor {
        AstKind::ParenthesizedExpression(_) | AstKind::ChainExpression(_) => {
          callee_id = ancestor.node_id();
        }
        AstKind::CallExpression(call) => return call.callee.node_id() == callee_id,
        AstKind::TaggedTemplateExpression(tagged) => return tagged.tag.node_id() == callee_id,
        _ => return false,
      }
    }
    false
  }

  pub fn init_dynamic_import_binding_usage_info(&mut self, import_record_idx: ImportRecordIdx) {
    let Some(last_visit_path) = self.visit_path.last() else {
      return;
    };
    let init_set = match last_visit_path {
      AstKind::StaticMemberExpression(expr) if expr.property.name == "then" => {
        self.init_dynamic_import_usage_with_static_member_expr(import_record_idx)
      }
      AstKind::AwaitExpression(_) => {
        self.extract_init_set_from_await_expr_ancestor(import_record_idx)
      }
      // e.g. `import('mod');`
      // init_set is empty, importee would be included if it has side effects
      AstKind::ExpressionStatement(_) if self.is_root_scope() => Some(FxHashSet::default()),
      _ => None,
    };

    let usage = match init_set {
      Some(init_set) => DynamicImportExportsUsage::Partial(init_set),
      None => DynamicImportExportsUsage::Complete,
    };
    self.dynamic_import_usage_info.dynamic_import_exports_usage.insert(import_record_idx, usage);
  }

  fn extract_init_set_from_await_expr_ancestor(
    &mut self,
    import_record_idx: ImportRecordIdx,
  ) -> Option<FxHashSet<CompactStr>> {
    let (_, ancestors) = self.visit_path.split_last()?;
    let ast_after_remove_paren_idx =
      ancestors.iter().rposition(|kind| !matches!(kind, AstKind::ParenthesizedExpression(_)))?;
    match self.visit_path[ast_after_remove_paren_idx] {
      // 1. const mod = await import('mod'); console.log(mod)
      // 2. const {a} = await import('mod'); a.something;
      AstKind::VariableDeclarator(var_decl) => {
        // parent of varDeclarator should be varDeclaration, so we should look for the parent of
        // parent
        let is_exported = matches!(
          self.visit_path.get(ast_after_remove_paren_idx.saturating_sub(2)),
          Some(AstKind::ExportDeclaration(_) | AstKind::ExportDefaultDeclaration(_))
        );
        self.update_dynamic_import_usage_info_from_binding_pattern(
          &var_decl.id,
          import_record_idx,
          is_exported,
        )
      }
      // 3. await import('mod');
      //   - return await import('mod'); // should consider it is completely used
      //   - () => await import('mod'); // should consider it is completely used
      // only side effects from `mod` is triggered
      AstKind::ExpressionStatement(_) => {
        // if the import is used in a function, we should consider it is completely used
        match self.visit_path.get(ast_after_remove_paren_idx.saturating_sub(1))? {
          AstKind::ReturnStatement(_) => None,
          AstKind::FunctionBody(_) => {
            match self.visit_path.get(ast_after_remove_paren_idx.saturating_sub(2))? {
              AstKind::ArrowFunctionExpression(expr) if expr.is_expression() => None,
              _ => Some(FxHashSet::default()),
            }
          }
          _ => Some(FxHashSet::default()),
        }
      }
      // 4. (await import('mod')).a
      kind if kind.is_member_expression_kind() => {
        let name: CompactStr =
          kind.as_member_expression_kind().unwrap().static_property_name()?.as_str()?.into();
        if self.dynamic_import_member_is_called(ast_after_remove_paren_idx) {
          self.record_dynamic_import_called_name(import_record_idx, &name);
        }
        Some(FxHashSet::from_iter([name]))
      }
      // for rest of the cases, just bailout, until we find other optimization could apply
      _ => None,
    }
  }

  fn init_dynamic_import_usage_with_static_member_expr(
    &mut self,
    import_record_idx: ImportRecordIdx,
  ) -> Option<FxHashSet<CompactStr>> {
    let parent_parent_idx = self.visit_path.len().saturating_sub(2);
    let parent_parent = self.visit_path.get(parent_parent_idx)?.as_call_expression()?;
    let first_arg = parent_parent.arguments.first()?;
    // A rest parameter, or a function's own `arguments`, reaches the namespace through an index
    // that the scan doesn't follow.
    let params = match first_arg {
      Argument::FunctionExpression(func) if !reads_own_arguments(func) => &func.params,
      Argument::ArrowFunctionExpression(func) => &func.params,
      _ => return None,
    };
    if params.rest.is_some() {
      return None;
    }
    let Some(dynamic_import_binding) = params.items.first() else {
      return Some(FxHashSet::default());
    };
    self.update_dynamic_import_usage_info_from_binding_pattern(
      &dynamic_import_binding.pattern,
      import_record_idx,
      false,
    )
  }

  fn update_dynamic_import_usage_info_from_binding_pattern(
    &mut self,
    binding_pattern: &ast::BindingPattern<'_>,
    import_record_idx: ImportRecordIdx,
    is_exported: bool,
  ) -> Option<FxHashSet<CompactStr>> {
    let symbol_id = match binding_pattern {
      ast::BindingPattern::BindingIdentifier(id) => {
        if is_exported {
          return None;
        }
        id.symbol_id()
      }
      // only care about first level destructuring, if it is nested just assume it is used
      ast::BindingPattern::ObjectPattern(obj) => {
        if is_exported && obj.rest.is_some() {
          // An exported rest binding exposes the remaining namespace to external consumers,
          // even without local references. See internal-docs/code-splitting/implementation.md.
          return None;
        }
        let mut set = FxHashSet::default();
        for binding in &obj.properties {
          let binding_name = match &binding.key {
            // for complex key pattern, just return `None` to bailout
            ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
            _ => return None,
          };
          let binding_symbol_id = match &binding.value {
            ast::BindingPattern::BindingIdentifier(id) => id.symbol_id(),
            _ => {
              // for complex alias pattern, assume the key is used
              // import('mod').then(({a: {b: {c: d}}}) => {})
              set.insert(binding_name.into());
              continue;
            }
          };
          let is_used = !self
            .result
            .symbol_ref_db
            .scoping()
            .get_resolved_reference_ids(binding_symbol_id)
            .is_empty();
          if is_exported || is_used {
            set.insert(binding_name.into());
          }
        }

        if let Some(rest) = &obj.rest {
          match &rest.argument {
            ast::BindingPattern::BindingIdentifier(id) => {
              if !self.register_dynamic_import_binding(id.symbol_id(), import_record_idx) {
                return None;
              }
            }
            // If the rest argument is not a BindingIdentifier, this is an unexpected case
            // because '...' must be followed by an identifier in declaration contexts.
            _ => unreachable!(),
          }
        }

        return Some(set);
      }
      ast::BindingPattern::ArrayPattern(_) | ast::BindingPattern::AssignmentPattern(_) => {
        // TODO: handle advance pattern
        return None;
      }
    };
    if !self.register_dynamic_import_binding(symbol_id, import_record_idx) {
      return None;
    }
    Some(FxHashSet::default())
  }

  /// Registers `symbol_id` as a binding of the namespace that `import_record_idx` imports, so that
  /// visiting its references counts what they read. A binding that another `import()` registered
  /// already, such as a `var` declared again, makes both imports completely used, since the scan
  /// can't tell which namespace a reference reads; this returns `false` then.
  fn register_dynamic_import_binding(
    &mut self,
    symbol_id: SymbolId,
    import_record_idx: ImportRecordIdx,
  ) -> bool {
    let info = &mut self.dynamic_import_usage_info;
    match info.dynamic_import_binding_to_import_record_idx.entry(symbol_id) {
      std::collections::hash_map::Entry::Occupied(registered) => {
        let registered = *registered.get();
        if registered == import_record_idx {
          return true;
        }
        info.dynamic_import_exports_usage.insert(registered, DynamicImportExportsUsage::Complete);
        false
      }
      std::collections::hash_map::Entry::Vacant(vacant) => {
        vacant.insert(import_record_idx);
        info
          .dynamic_import_binding_reference_id
          .extend(self.result.symbol_ref_db.scoping().get_resolved_reference_ids(symbol_id));
        true
      }
    }
  }
}

/// Whether `function` reads its own `arguments`, in its parameters or its body. A nested function
/// has `arguments` of its own, and an arrow function shares its parent's. A binding that sloppy
/// code names `arguments` counts too, which only makes the usage complete when it needn't be.
fn reads_own_arguments(function: &ast::Function<'_>) -> bool {
  struct ArgumentsReads(bool);
  impl<'a> VisitJs<'a> for ArgumentsReads {
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
      self.0 |= it.name == "arguments";
    }

    fn visit_function(&mut self, _it: &ast::Function<'a>, _flags: ScopeFlags) {}
  }
  let mut reads = ArgumentsReads(false);
  reads.visit_formal_parameters(&function.params);
  if let Some(body) = &function.body {
    reads.visit_function_body(body);
  }
  reads.0
}
