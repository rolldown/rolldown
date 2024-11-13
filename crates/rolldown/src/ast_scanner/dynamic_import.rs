use oxc::{
  ast::{
    ast::{self, Argument, IdentifierReference},
    AstKind,
  },
  span::CompactStr,
};
use rolldown_common::{dynamic_import_usage::DynamicImportExportsUsage, ImportRecordIdx};
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

    let reference =
      self.scopes.references.get(ident.reference_id()).expect("should have reference");

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
    import_record_id: ImportRecordIdx,
  ) -> Option<FxHashSet<CompactStr>> {
    let ancestor_len = self.visit_path.len();
    let parent = self.visit_path.last()?.as_member_expression()?;
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
    let symbol_id = match &dynamic_import_binding.pattern.kind {
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
          let is_used = !self.scopes.resolved_references[binding_symbol_id].is_empty();
          if is_used {
            set.insert(binding_name.into());
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
      .extend(self.scopes.resolved_references[symbol_id].iter());
    Some(FxHashSet::default())
  }
}
