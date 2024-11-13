use std::os::unix::process::parent_id;

use oxc::{
  ast::{
    ast::{Argument, IdentifierReference, UnaryOperator},
    AstKind,
  },
  semantic::{ReferenceId, SymbolFlags, SymbolId},
  span::{CompactStr, Span},
};
use rolldown_common::{ImportRecordIdx, Specifier};
use rustc_hash::{FxHashMap, FxHashSet};

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
  ) -> Option<()> {
    let ancestor_len = self.visit_path.len();
    let parent = self.visit_path.last()?.as_member_expression()?;
    let parent = match parent {
      oxc::ast::ast::MemberExpression::StaticMemberExpression(parent) => parent,
      _ => return None,
    };
    if parent.property.name != "then" {
      return None;
    }
    let parent_parent = self.visit_path.get(ancestor_len - 2)?.as_call_expression()?;
    let first_arg = parent_parent.arguments.get(0)?;
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
      oxc::ast::ast::BindingPatternKind::BindingIdentifier(id) => id.symbol_id.get()?,
      oxc::ast::ast::BindingPatternKind::ObjectPattern(_)
      | oxc::ast::ast::BindingPatternKind::ArrayPattern(_)
      | oxc::ast::ast::BindingPatternKind::AssignmentPattern(_) => {
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
    Some(())
  }
}

#[derive(Default)]
pub(crate) struct DynamicImportUsageInfo {
  /// e.g
  /// ```js
  /// import('mod').then(mod => {
  ///   mod.test // ref1
  ///   mod // ref2
  /// })
  /// ```
  /// record all these dynamic import binding reference id
  /// used for analyze how dynamic import binding is used (partially or fully used),
  pub dynamic_import_binding_reference_id: FxHashSet<ReferenceId>,
  pub dynamic_import_binding_to_import_record_id: FxHashMap<SymbolId, ImportRecordIdx>,
  pub dynamic_import_exports_usage: FxHashMap<ImportRecordIdx, DynamicImportExportsUsage>,
}

#[derive(Debug, Clone)]
pub enum DynamicImportExportsUsage {
  Complete,
  Partial(FxHashSet<CompactStr>),
  /// This is used for insert a single export to Partial
  /// so that we don't need to create `FxHashSet` for each insertion
  Single(CompactStr),
}

impl DynamicImportExportsUsage {
  pub fn merge(&mut self, other: Self) {
    match (&mut *self, other) {
      (Self::Complete, _) => {}
      (_, Self::Complete) => {
        *self = DynamicImportExportsUsage::Complete;
      }
      (Self::Partial(lhs), rhs) => {
        match rhs {
          DynamicImportExportsUsage::Complete => unreachable!(),
          DynamicImportExportsUsage::Partial(rhs) => {
            lhs.extend(rhs);
          }
          DynamicImportExportsUsage::Single(name) => {
            lhs.insert(name);
          }
        };
      }
      (Self::Single(name), rhs) => {
        let set = match rhs {
          DynamicImportExportsUsage::Complete => unreachable!(),
          DynamicImportExportsUsage::Partial(mut rhs) => {
            rhs.insert(name.clone());
            rhs
          }
          DynamicImportExportsUsage::Single(rhs) => {
            let mut set = FxHashSet::default();
            set.insert(rhs);
            set.insert(name.clone());
            set
          }
        };
        *self = DynamicImportExportsUsage::Partial(set);
      }
    };
  }
}
