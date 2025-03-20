use oxc::{
  ast::{
    AstKind,
    ast::{self, Argument, IdentifierReference},
  },
  span::CompactStr,
};
use rolldown_common::{ImportRecordIdx, dynamic_import_usage::DynamicImportExportsUsage};
use rustc_hash::FxHashSet;

use super::AstScanner;

impl<'me, 'ast: 'me> AstScanner<'me, 'ast> {
  pub fn update_dynamic_import_binding_usage_info(
    &mut self,
    ident: &IdentifierReference,
  ) -> Option<()> {
    if !self
      .dynamic_import_usage_info
      .dynamic_import_binding_reference_id
      .contains(&ident.reference_id())
    {
      return None;
    }

    let reference = self.result.symbol_ref_db.get_reference(ident.reference_id());

    // panic because if program reached here, means the BindingIdentifier has referenced the
    // IdentifierReference, but IdentifierReference did not saved the related `SymbolId`
    // Something wrong with semantic analyze
    let symbol_id = reference.symbol_id().expect("should have symbol id");
    let parent = self.visit_path.last()?;
    // if the property could be converted as a static property name, e.g.
    // a.b // static
    // a.['b'] // static
    // a[b] // dynamic
    let partial_name = match parent {
      AstKind::MemberExpression(expr) => expr.static_property_name(),
      _ => None,
    };
    let rec_idx =
      *self.dynamic_import_usage_info.dynamic_import_binding_to_import_record_id.get(&symbol_id)?;

    match self.dynamic_import_usage_info.dynamic_import_exports_usage.entry(rec_idx) {
      std::collections::hash_map::Entry::Occupied(mut occ) => match partial_name {
        Some(name) => occ.get_mut().merge(DynamicImportExportsUsage::Single(name.into())),
        None => occ.get_mut().merge(DynamicImportExportsUsage::Complete),
      },
      std::collections::hash_map::Entry::Vacant(vac) => match partial_name {
        Some(name) => {
          vac.insert(DynamicImportExportsUsage::Single(name.into()));
        }
        None => {
          vac.insert(DynamicImportExportsUsage::Complete);
        }
      },
    }

    None
  }

  pub fn init_dynamic_import_binding_usage_info(
    &mut self,
    import_record_idx: ImportRecordIdx,
  ) -> Option<()> {
    let ancestor_len = self.visit_path.len();
    let init_set = match self.visit_path.last()? {
      AstKind::MemberExpression(member_expr) => self.init_dynamic_import_usage_with_member_expr(
        member_expr,
        ancestor_len,
        import_record_idx,
      ),
      AstKind::AwaitExpression(_) => {
        self.extract_init_set_from_await_expr_ancestor(import_record_idx)
      }
      // e.g. `import('mod');`
      // init_set is empty, importee would be included if it has side effects
      AstKind::ExpressionStatement(_) if self.is_root_scope() => Some(FxHashSet::default()),
      _ => None,
    };

    match init_set {
      Some(init_set) => {
        self
          .dynamic_import_usage_info
          .dynamic_import_exports_usage
          .insert(import_record_idx, DynamicImportExportsUsage::Partial(init_set));
      }
      None => {
        self
          .dynamic_import_usage_info
          .dynamic_import_exports_usage
          .insert(import_record_idx, DynamicImportExportsUsage::Complete);
      }
    };
    None
  }

  fn extract_init_set_from_await_expr_ancestor(
    &mut self,
    import_record_idx: ImportRecordIdx,
  ) -> Option<std::collections::HashSet<CompactStr, rustc_hash::FxBuildHasher>> {
    let remove_paren = self
      .visit_path
      .iter()
      .rev()
      .skip(1)
      .find(|kind| !matches!(kind, AstKind::ParenthesizedExpression(_)))?;
    match remove_paren {
      // 1. const mod = await import('mod'); console.log(mod)
      // 2. const {a} = await import('mod'); a.something;
      AstKind::VariableDeclarator(var_decl) => {
        self.update_dynamic_import_usage_info_from_binding_pattern(&var_decl.id, import_record_idx)
      }
      // 3. await import('mod');
      // only side effects from `mod` is triggered
      AstKind::ExpressionStatement(_) => Some(FxHashSet::default()),
      // 4. (await import('mod')).a
      AstKind::MemberExpression(expr) => {
        Some(FxHashSet::from_iter([expr.static_property_name()?.into()]))
      }
      // for rest of the cases, just bailout, until we find other optimization could apply
      _ => None,
    }
  }

  fn init_dynamic_import_usage_with_member_expr(
    &mut self,
    parent: &ast::MemberExpression<'ast>,
    ancestor_len: usize,
    import_record_id: ImportRecordIdx,
  ) -> Option<FxHashSet<CompactStr>> {
    let ast::MemberExpression::StaticMemberExpression(parent) = parent else {
      return None;
    };
    if parent.property.name != "then" {
      return None;
    }
    let parent_parent = self.visit_path.get(ancestor_len - 2)?.as_call_expression()?;
    let first_arg = parent_parent.arguments.first()?;
    let dynamic_import_binding = match first_arg {
      Argument::FunctionExpression(func) => func.params.items.first()?,
      Argument::ArrowFunctionExpression(func) => func.params.items.first()?,
      _ => {
        return None;
      }
    };
    // for now only handle
    // ```js
    // import('mod').then(mod => {
    //   mod.a;
    //   mod;
    // })
    // ```
    self.update_dynamic_import_usage_info_from_binding_pattern(
      &dynamic_import_binding.pattern,
      import_record_id,
    )
  }

  fn update_dynamic_import_usage_info_from_binding_pattern(
    &mut self,
    binding_pattern: &ast::BindingPattern<'_>,
    import_record_id: ImportRecordIdx,
  ) -> Option<FxHashSet<CompactStr>> {
    let symbol_id = match &binding_pattern.kind {
      ast::BindingPatternKind::BindingIdentifier(id) => id.symbol_id(),
      // only care about first level destructuring, if it is nested just assume it is used
      ast::BindingPatternKind::ObjectPattern(obj) => {
        let mut set = FxHashSet::default();
        for binding in &obj.properties {
          let binding_name = match &binding.key {
            // for complex key pattern, just return `None` to bailout
            ast::PropertyKey::StaticIdentifier(id) => id.name.as_str(),
            _ => return None,
          };
          let binding_symbol_id = match &binding.value.kind {
            ast::BindingPatternKind::BindingIdentifier(id) => id.symbol_id(),
            _ => {
              // for complex alias pattern, assume the key is used
              // import('mod').then(({a: {b: {c: d}}}) => {})
              set.insert(binding_name.into());
              continue;
            }
          };
          let is_used =
            !self.result.symbol_ref_db.get_resolved_reference_ids(binding_symbol_id).is_empty();
          if is_used {
            set.insert(binding_name.into());
          }
        }

        if let Some(rest) = &obj.rest {
          match &rest.argument.kind {
            ast::BindingPatternKind::BindingIdentifier(id) => {
              let symbol_id = id.symbol_id();
              self
                .dynamic_import_usage_info
                .dynamic_import_binding_to_import_record_id
                .insert(symbol_id, import_record_id);
              self
                .dynamic_import_usage_info
                .dynamic_import_binding_reference_id
                .extend(self.result.symbol_ref_db.get_resolved_reference_ids(symbol_id));
            }
            // If the rest argument is not a BindingIdentifier, this is an unexpected case
            // because '...' must be followed by an identifier in declaration contexts.
            _ => unreachable!(),
          }
        }

        return Some(set);
      }
      ast::BindingPatternKind::ArrayPattern(_) | ast::BindingPatternKind::AssignmentPattern(_) => {
        // TODO: handle advance pattern
        return None;
      }
    };
    self
      .dynamic_import_usage_info
      .dynamic_import_binding_to_import_record_id
      .insert(symbol_id, import_record_id);
    self
      .dynamic_import_usage_info
      .dynamic_import_binding_reference_id
      .extend(self.result.symbol_ref_db.get_resolved_reference_ids(symbol_id));
    Some(FxHashSet::default())
  }
}
