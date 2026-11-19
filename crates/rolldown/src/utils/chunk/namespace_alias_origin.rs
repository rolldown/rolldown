use rolldown_common::{ExportsKind, Module, ModuleIdx, NamespaceAlias};
use rustc_hash::FxHashSet;

use crate::stages::link_stage::LinkStageOutput;

/// The module that gives the value of a namespace alias. The linker makes a namespace alias for an
/// import that it cannot bind to a symbol. Then the output reads the import as a property of a
/// namespace object (`ns.name`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamespaceAliasOrigin {
  /// A CommonJS module gives the property. This is true in these conditions:
  /// - The namespace is the namespace of an import record, and the importee is CommonJS.
  /// - The namespace is the namespace of a barrel, and the barrel gets the property through
  ///   `export *` from CommonJS modules only.
  ///
  /// Node reads such a property one time, when it imports the CommonJS module. The value is the
  /// CommonJS module, if the output can tell which one module gives the property.
  CommonJs(Option<ModuleIdx>),
  /// One external module gives the property. This is true in these conditions:
  /// - The namespace is the namespace of the external module.
  /// - The namespace is the namespace of a barrel, and the barrel gets the property through
  ///   `export *` from this external module only, and from no CommonJS module.
  ///
  /// The property is a live binding of an ES module.
  External(ModuleIdx),
  /// The output cannot tell which module gives the property. The barrel gets it through
  /// `export *` from two or more such modules, or the property is `default`, which `export *` does
  /// not forward.
  Unknown,
}

/// Returns the module that gives the value of `ns_alias`. See [`NamespaceAliasOrigin`].
pub fn namespace_alias_origin(
  link_output: &LinkStageOutput,
  ns_alias: &NamespaceAlias,
) -> NamespaceAliasOrigin {
  let namespace_ref = ns_alias.namespace_ref;
  let owner = match &link_output.module_table[namespace_ref.owner] {
    Module::External(_) => return NamespaceAliasOrigin::External(namespace_ref.owner),
    Module::Normal(owner) => owner,
  };
  let is_commonjs = |module_idx: ModuleIdx| {
    link_output.module_table[module_idx]
      .as_normal()
      .is_some_and(|module| matches!(module.exports_kind, ExportsKind::CommonJs))
  };
  if namespace_ref != owner.namespace_object_ref {
    // The linker uses the namespace of an import record only when the importee is CommonJS.
    let commonjs_importee = owner.import_records.iter().find_map(|record| {
      (record.namespace_ref == namespace_ref)
        .then_some(record.resolved_module)
        .flatten()
        .filter(|&module_idx| is_commonjs(module_idx))
    });
    return commonjs_importee.map_or(NamespaceAliasOrigin::Unknown, |module_idx| {
      NamespaceAliasOrigin::CommonJs(Some(module_idx))
    });
  }
  if let Some(export) = link_output.metas[owner.idx].resolved_exports.get(&ns_alias.property_name)
    && export.came_from_commonjs
  {
    // The export is a binding of the CommonJS module. Other CommonJS modules can give the property
    // at run time (`cjs_conflicting_symbol_refs`).
    let module_idx = export.symbol_ref.owner;
    let has_one_source =
      export.cjs_conflicting_symbol_refs.as_ref().is_none_or(|refs| refs.is_empty());
    return NamespaceAliasOrigin::CommonJs(
      (has_one_source && is_commonjs(module_idx)).then_some(module_idx),
    );
  }
  if ns_alias.property_name == "default" {
    return NamespaceAliasOrigin::Unknown;
  }

  // The linker found no binding of the property in the barrel, so the property comes from an
  // `export *` source that has no known names: a CommonJS module or an external module. The walk
  // follows `export *` through ES modules, as `determine_has_dynamic_exports` does.
  let mut externals = vec![];
  let mut commonjs_modules = vec![];
  let mut visited = FxHashSet::from_iter([owner.idx]);
  let mut stack = vec![owner.idx];
  while let Some(module_idx) = stack.pop() {
    let Some(module) = link_output.module_table[module_idx].as_normal() else {
      continue;
    };
    for importee_idx in module.star_export_module_ids() {
      if !visited.insert(importee_idx) {
        continue;
      }
      match &link_output.module_table[importee_idx] {
        Module::External(_) => externals.push(importee_idx),
        Module::Normal(importee) if matches!(importee.exports_kind, ExportsKind::CommonJs) => {
          commonjs_modules.push(importee_idx);
        }
        Module::Normal(_) => stack.push(importee_idx),
      }
    }
  }
  match (commonjs_modules.as_slice(), externals.as_slice()) {
    ([module_idx], []) => NamespaceAliasOrigin::CommonJs(Some(*module_idx)),
    ([_, _, ..], []) => NamespaceAliasOrigin::CommonJs(None),
    ([], [external]) => NamespaceAliasOrigin::External(*external),
    _ => NamespaceAliasOrigin::Unknown,
  }
}
