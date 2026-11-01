//! Scan-level verdicts on the link-time cell bypass certificate
//! (`native_records::native_cell_evaluation_dependencies`).
//!
//! The HMR stage has no link output, so it classifies every static import record whose donor
//! re-exports a native cell:
//!
//! - [`Verdict::Bypass`]: every condition the certificate checks is decided by scan data and
//!   holds, so the full build bound the importer straight to the cell records and never
//!   evaluated the donor's closure. A patch must route the same way.
//! - [`Verdict::Evaluate`]: a condition the certificate requires certainly fails, so the full
//!   build evaluated the donor like any other import.
//! - [`Verdict::Uncertain`]: anything else (namespace member reads, which need link-time member
//!   resolution, cycles, wrapping, re-exports, size limits). A patch cannot tell which shape the
//!   page holds, so edits that touch such a candidate reload.
//!
//! The certificate also binds an import straight to cells that its donor only forwards through
//! re-exports, so such an import is a candidate too, decided along the ways that the link may
//! resolve its bindings ([`forwarded_lead`]): a patch routes it as it routes an import of a
//! donor's own cells, and the evaluation of what an entry ran follows it. An undecided candidate
//! whose donor every entry runs through other imports settles as an evaluation: whichever shape
//! the page holds, the donor runs.

use rolldown_common::{
  EcmaModuleAstUsage, ExportsKind, ImportKind, ImportRecordIdx, ImportRecordMeta, IndexModules,
  ModuleIdx, NormalModule, Specifier, StmtInfos, SymbolOrMemberExprRef, SymbolRef,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  type_alias::{IndexEcmaAst, IndexStmtInfos},
  utils::{
    import_options::is_json_module,
    native_records::{
      CELL_BYPASS_MAX_EDGES as MAX_EDGES, CELL_BYPASS_MAX_MODULES as MAX_MODULES,
      is_literal_record, is_native_cell,
    },
  },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
  Bypass,
  Evaluate,
  Uncertain,
}

pub struct Route {
  pub importer: ModuleIdx,
  pub record: ImportRecordIdx,
  pub donor: ModuleIdx,
  pub verdict: Verdict,
  /// Every module statically reachable from the donor, through records of any kind, sorted.
  /// Empty for [`Verdict::Evaluate`] and in [`evaluation_routes`], whose readers don't need it.
  pub closure: Vec<ModuleIdx>,
  /// For [`Verdict::Bypass`]: imported name to its cell and the cell's export name.
  pub bindings: FxHashMap<String, (ModuleIdx, String)>,
  /// For [`Verdict::Bypass`]: the cells and literal records of the bindings that the importer
  /// reads, which its record evaluates in place of the donor. It imports nothing for the others.
  pub live_owners: Vec<ModuleIdx>,
}

/// The native cell and its export name that a donor's export leads to.
type CellOf = fn(&IndexModules, &NormalModule, &str) -> Option<(ModuleIdx, String)>;

fn cell_export(
  modules: &IndexModules,
  donor: &NormalModule,
  name: &str,
) -> Option<(ModuleIdx, String)> {
  let export = donor.named_exports.get(name)?;
  let import = donor.named_imports.get(&export.referenced)?;
  let Specifier::Literal(cell_name) = &import.imported else {
    return None;
  };
  let cell = modules[donor.import_records[import.record_idx].resolved_module?].as_normal()?;
  (is_native_cell(cell.id.as_str())
    && cell.import_records.is_empty()
    && cell
      .named_exports
      .get(cell_name.as_str())
      .is_some_and(|export| !cell.named_imports.contains_key(&export.referenced)))
  .then(|| (cell.idx, cell_name.to_string()))
}

/// [`cell_export`] of a module that `name` leads to through named re-exports and `export *`.
fn forwarded_cell_export(
  modules: &IndexModules,
  donor: &NormalModule,
  name: &str,
) -> Option<(ModuleIdx, String)> {
  match forwarded_lead(modules, donor, name) {
    Lead::Cell(cell, cell_name) => Some((cell, cell_name)),
    Lead::Elsewhere | Lead::Undecided => None,
  }
}

/// Where an export leads through named re-exports and `export *`.
enum Lead {
  /// To the export of a native cell, by the cell's name for it.
  Cell(ModuleIdx, String),
  /// To no native cell.
  Elsewhere,
  /// Further than the scan follows it, which may be to a native cell.
  Undecided,
}

/// Where the export `name` of `donor` leads, along every way that the link may resolve it, which
/// for a name other than `default` runs through `export *` too. A way ends elsewhere only at a
/// declaration outside a cell, at a namespace object, or at an ES module that doesn't export the
/// name. A way past an unresolved record, an external, a module that isn't an ES module or the
/// budget leaves the lead undecided.
fn forwarded_lead(modules: &IndexModules, donor: &NormalModule, name: &str) -> Lead {
  let mut visited = FxHashSet::default();
  let mut pending = vec![(donor.idx, name.to_string())];
  let mut undecided = false;
  while let Some((idx, name)) = pending.pop() {
    if !visited.insert((idx, name.clone())) {
      continue;
    }
    if visited.len() > MAX_MODULES {
      return Lead::Undecided;
    }
    let Some(module) =
      modules[idx].as_normal().filter(|module| module.exports_kind == ExportsKind::Esm)
    else {
      undecided = true;
      continue;
    };
    if let Some((cell, cell_name)) = cell_export(modules, module, &name) {
      return Lead::Cell(cell, cell_name);
    }
    let Some(export) = module.named_exports.get(name.as_str()) else {
      if name != "default" {
        pending.extend(module.star_export_module_ids().map(|star| (star, name.clone())));
      }
      continue;
    };
    if let Some(import) = module.named_imports.get(&export.referenced)
      && let Specifier::Literal(imported) = &import.imported
    {
      match module.import_records[import.record_idx].resolved_module {
        Some(next) => pending.push((next, imported.to_string())),
        None => undecided = true,
      }
    }
  }
  if undecided { Lead::Undecided } else { Lead::Elsewhere }
}

/// Whether an export of `donor`'s namespace may lead to a native cell ([`forwarded_lead`]): one
/// that it names, one that an `export *` of it may forward, or any, when its `export *` records
/// lead further than the scan follows.
fn namespace_may_lead_to_cell(modules: &IndexModules, donor: &NormalModule) -> bool {
  let mut names = FxHashSet::default();
  let mut visited = FxHashSet::default();
  let mut pending = vec![donor.idx];
  while let Some(idx) = pending.pop() {
    if !visited.insert(idx) {
      continue;
    }
    let Some(module) =
      modules[idx].as_normal().filter(|module| module.exports_kind == ExportsKind::Esm)
    else {
      return true;
    };
    if visited.len() > MAX_MODULES {
      return true;
    }
    names.extend(
      module
        .named_exports
        .keys()
        .map(|name| name.as_str())
        .filter(|name| idx == donor.idx || *name != "default"),
    );
    pending.extend(module.star_export_module_ids());
  }
  names.into_iter().any(|name| !matches!(forwarded_lead(modules, donor, name), Lead::Elsewhere))
}

/// Whether `name`, followed through named re-exports, is declared by a module that is not a
/// native cell. Star re-exports and unresolved records stay undecided.
fn declared_outside_cells(modules: &IndexModules, donor: &NormalModule, name: &str) -> bool {
  let mut module = donor;
  let mut name = name.to_string();
  for _ in 0..MAX_MODULES {
    let Some(export) = module.named_exports.get(name.as_str()) else {
      return false;
    };
    let Some(import) = module.named_imports.get(&export.referenced) else {
      return !is_native_cell(module.id.as_str());
    };
    let Specifier::Literal(imported) = &import.imported else {
      return false;
    };
    let Some(next) = module.import_records[import.record_idx]
      .resolved_module
      .and_then(|idx| modules[idx].as_normal())
    else {
      return false;
    };
    module = next;
    name = imported.to_string();
  }
  false
}

/// The literal record (`native_records::is_literal_record`) other than the donor that declares
/// `name`, followed through named re-exports, with its own export name.
fn literal_owner(
  modules: &IndexModules,
  asts: &IndexEcmaAst,
  donor: &NormalModule,
  name: &str,
) -> Option<(ModuleIdx, String)> {
  let mut module = donor;
  let mut name = name.to_string();
  for _ in 0..MAX_MODULES {
    let export = module.named_exports.get(name.as_str())?;
    let Some(import) = module.named_imports.get(&export.referenced) else {
      return (module.idx != donor.idx
        && !is_native_cell(module.id.as_str())
        && is_literal_record(module, asts.get(module.idx)?.as_ref()?.program()))
      .then_some((module.idx, name));
    };
    let Specifier::Literal(imported) = &import.imported else {
      return None;
    };
    module = module.import_records[import.record_idx]
      .resolved_module
      .and_then(|idx| modules[idx].as_normal())?;
    name = imported.to_string();
  }
  None
}

/// Whether `module` re-exports one of its imports, whose resolution the scan cannot follow.
fn reexports_import(module: &NormalModule) -> bool {
  module.star_export_module_ids().next().is_some()
    || module
      .named_exports
      .values()
      .any(|export| module.named_imports.contains_key(&export.referenced))
}

/// Whether any top-level statement of the importer reads `symbol`, including from nested
/// scopes: the scan's counterpart of a resolved reference. The cached scan keeps statement
/// infos but not scoping.
fn referenced(statements: &StmtInfos, symbol: SymbolRef) -> bool {
  statements.iter().any(|statement| {
    statement.referenced_symbols.iter().any(|reference| *reference.symbol_ref() == symbol)
  })
}

/// The verdicts on the static imports that bind a native cell or may: one of their donor's own,
/// or one that their donor forwards through re-exports, or may, where the scan doesn't follow
/// every way to its end. A route through an `export *` is never a bypass, since only the link
/// resolves a star's names. An undecided route whose donor every one of the user-defined
/// `entries` runs through other imports is an evaluation ([`settle_run_donors`]).
pub fn routes(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  entries: &FxHashSet<ModuleIdx>,
) -> Vec<Route> {
  candidate_routes(modules, statements, asts, entries, true)
}

/// [`routes`] without their closures, which only patches read.
pub fn evaluation_routes(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  entries: &FxHashSet<ModuleIdx>,
) -> Vec<Route> {
  candidate_routes(modules, statements, asts, entries, false)
}

fn candidate_routes(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  entries: &FxHashSet<ModuleIdx>,
  closures: bool,
) -> Vec<Route> {
  let mut has_native_cells = false;
  for module in modules.iter().filter_map(|module| module.as_normal()) {
    if is_native_cell(module.id.as_str()) {
      has_native_cells = true;
      break;
    }
  }
  if !has_native_cells {
    return Vec::new();
  }
  let mut required = FxHashSet::default();
  let mut cell_donors = FxHashSet::default();
  for module in modules.iter().filter_map(|module| module.as_normal()) {
    for record in module.import_records.iter() {
      if let Some(idx) = record.resolved_module {
        if record.kind == ImportKind::Require {
          required.insert(idx);
        }
        if modules[idx].as_normal().is_some_and(|m| is_native_cell(m.id.as_str())) {
          cell_donors.insert(module.idx);
        }
      }
    }
  }
  if cell_donors.is_empty() {
    return Vec::new();
  }
  let mut routes = Vec::new();
  for importer in modules.iter().filter_map(|module| module.as_normal()) {
    for (record_idx, record) in importer.import_records.iter_enumerated() {
      if record.kind != ImportKind::Import || record.meta.contains(ImportRecordMeta::IsExportStar) {
        continue;
      }
      let Some(donor) = record.resolved_module.and_then(|idx| modules[idx].as_normal()) else {
        continue;
      };
      let imports = importer
        .named_imports
        .iter()
        .filter(|(_, import)| import.record_idx == record_idx)
        .collect::<Vec<_>>();
      let binds_own_cell = cell_donors.contains(&donor.idx)
        && imports.iter().any(|(_, import)| match &import.imported {
          Specifier::Literal(name) => cell_export(modules, donor, name).is_some(),
          Specifier::Star => {
            donor.named_exports.keys().any(|name| cell_export(modules, donor, name).is_some())
          }
        });
      // An import that may bind a cell that its donor forwards gets a verdict too, a bypass only
      // where the scan follows a binding all the way to its cell.
      let cell_of: CellOf = if binds_own_cell {
        cell_export
      } else if reexports_import(donor)
        && imports.iter().any(|(_, import)| match &import.imported {
          Specifier::Literal(name) => {
            !matches!(forwarded_lead(modules, donor, name), Lead::Elsewhere)
          }
          Specifier::Star => namespace_may_lead_to_cell(modules, donor),
        })
      {
        forwarded_cell_export
      } else {
        continue;
      };
      routes.push(classify(
        modules, statements, asts, &required, importer, record_idx, donor, &imports, cell_of,
      ));
    }
  }
  settle_run_donors(modules, statements, asts, entries, &mut routes);
  if closures {
    for route in routes.iter_mut().filter(|route| route.verdict != Verdict::Evaluate) {
      route.closure = static_closure(modules, route.donor);
    }
  }
  routes
}

/// Settles as an evaluation each [`Verdict::Uncertain`] route whose donor every one of `entries`
/// runs ([`resident`]). Whichever way the link bound such an import, the donor runs on every
/// page: a patch's factory can load it, and the evaluation of what an entry ran counts it as run
/// either way.
fn settle_run_donors(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  entries: &FxHashSet<ModuleIdx>,
  routes: &mut [Route],
) {
  if !routes.iter().any(|route| route.verdict == Verdict::Uncertain) {
    return;
  }
  let run = resident(modules, statements, asts, entries, routes);
  for route in routes.iter_mut() {
    if route.verdict == Verdict::Uncertain && run.contains(&route.donor) {
      route.verdict = Verdict::Evaluate;
    }
  }
}

/// The modules that every one of the user-defined `entries` runs through imports that certainly
/// evaluate their targets ([`certain_successors`]), along `routes`: what any page ran, whichever
/// of the entries it loaded and whichever way the link bound an undecided route.
pub fn resident(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  entries: &FxHashSet<ModuleIdx>,
  routes: &[Route],
) -> FxHashSet<ModuleIdx> {
  let by_record = routes
    .iter()
    .map(|route| ((route.importer, route.record), (route.verdict, route.live_owners.as_slice())))
    .collect::<FxHashMap<_, _>>();
  let mut successors = FxHashMap::default();
  let mut run: Option<FxHashSet<ModuleIdx>> = None;
  for &entry in entries {
    let mut visited = FxHashSet::default();
    let mut pending = vec![entry];
    while let Some(idx) = pending.pop() {
      if visited.insert(idx) {
        pending.extend(
          successors
            .entry(idx)
            .or_insert_with(|| certain_successors(modules, statements, asts, &by_record, idx))
            .iter()
            .copied(),
        );
      }
    }
    run = Some(match run {
      Some(run) => run.intersection(&visited).copied().collect(),
      None => visited,
    });
  }
  run.unwrap_or_default()
}

/// The modules that evaluating `idx` certainly evaluates through its static imports: what a
/// [`Verdict::Bypass`] record evaluates in place of its donor, and the targets of other records,
/// but not those of an undecided route, which may only bind cells, of an `import defer`, which
/// links its target without evaluating it, of an `export *` whose evaluation edge a native build
/// may omit (`native_records::omittable_star_evaluation`), nor of an import that development
/// output drops as unused (`hmr::utils::unused_side_effect_free_import`). Nothing without the
/// module's AST.
fn certain_successors(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  by_record: &FxHashMap<(ModuleIdx, ImportRecordIdx), (Verdict, &[ModuleIdx])>,
  idx: ModuleIdx,
) -> Vec<ModuleIdx> {
  let (Some(module), Some(ast)) =
    (modules[idx].as_normal(), asts.get(idx).and_then(Option::as_ref))
  else {
    return Vec::new();
  };
  let deferred = crate::hmr::utils::deferred_records(module, ast.program());
  // The records that bind a name, and those of which the module reads or re-exports one.
  let read = statements.get(idx).map(|statements| {
    statements
      .iter()
      .flat_map(|statement| statement.referenced_symbols.iter())
      .map(|reference| *reference.symbol_ref())
      .collect::<FxHashSet<_>>()
  });
  let reexported =
    module.named_exports.values().map(|export| export.referenced).collect::<FxHashSet<_>>();
  let mut bound = FxHashSet::default();
  let mut used = FxHashSet::default();
  for (binding, import) in &module.named_imports {
    bound.insert(import.record_idx);
    if reexported.contains(binding) || read.as_ref().is_some_and(|read| read.contains(binding)) {
      used.insert(import.record_idx);
    }
  }
  let mut successors = Vec::new();
  for (record_idx, record) in module.import_records.iter_enumerated() {
    let Some(target) = record.resolved_module.and_then(|target| modules[target].as_normal()) else {
      continue;
    };
    if record.kind != ImportKind::Import || deferred.contains(&record_idx) {
      continue;
    }
    let evaluates = if record.meta.contains(ImportRecordMeta::IsExportStar) {
      !target.import_records.is_empty()
        || target.side_effects.has_side_effects()
        || target.meta.has_eval()
    } else {
      module.meta.has_eval()
        || record.meta.contains(ImportRecordMeta::IsReExportOnly)
        || target.side_effects.has_side_effects()
        || !bound.contains(&record_idx)
        || used.contains(&record_idx)
    };
    match by_record.get(&(idx, record_idx)) {
      Some((Verdict::Bypass, owners)) => successors.extend(owners.iter().copied()),
      Some((Verdict::Uncertain, _)) => {}
      _ if evaluates => successors.push(target.idx),
      _ => {}
    }
  }
  successors
}

/// Every module statically reachable from `donor`, through records of any kind, sorted.
fn static_closure(modules: &IndexModules, donor: ModuleIdx) -> Vec<ModuleIdx> {
  let mut closure = FxHashSet::default();
  let mut pending = vec![donor];
  while let Some(idx) = pending.pop() {
    if closure.insert(idx)
      && let Some(module) = modules[idx].as_normal()
    {
      pending.extend(module.import_records.iter().filter_map(|record| record.resolved_module));
    }
  }
  let mut closure = closure.into_iter().collect::<Vec<_>>();
  closure.sort_unstable();
  closure
}

#[expect(clippy::too_many_arguments)]
fn classify(
  modules: &IndexModules,
  statements: &IndexStmtInfos,
  asts: &IndexEcmaAst,
  required: &FxHashSet<ModuleIdx>,
  importer: &NormalModule,
  record: ImportRecordIdx,
  donor: &NormalModule,
  imports: &[(&rolldown_common::SymbolRef, &rolldown_common::NamedImport)],
  cell_of: CellOf,
) -> Route {
  let reexport_only =
    importer.import_records[record].meta.contains(ImportRecordMeta::IsReExportOnly);
  let mut evaluate = importer.meta.has_eval()
    || importer.exports_kind == ExportsKind::CommonJs
    || is_json_module(importer)
    || reexport_only;
  let mut uncertain = importer.exports_kind != ExportsKind::Esm
    || required.contains(&importer.idx)
    || reexports_import(importer);
  let mut bindings = FxHashMap::default();
  let mut live_owners = Vec::new();
  let statements = statements.get(importer.idx);
  let mut live = false;
  let mut uncertain_live = false;
  for (binding, import) in imports {
    let name = match &import.imported {
      Specifier::Literal(name) if name == "default" => {
        evaluate = true;
        continue;
      }
      Specifier::Literal(name) => Some(name.as_str()),
      Specifier::Star => None,
    };
    let cell = name.and_then(|name| cell_of(modules, donor, name));
    let literal = name.and_then(|name| literal_owner(modules, asts, donor, name));
    let owner = cell.as_ref().or(literal.as_ref());
    if let (Some(name), Some(owner)) = (name, owner) {
      bindings.insert(name.to_string(), owner.clone());
    }
    let Some(statements) = statements else {
      uncertain = true;
      uncertain_live = true;
      continue;
    };
    if !referenced(statements, **binding) {
      if !reexport_only
        && importer.named_exports.values().any(|export| export.referenced == **binding)
      {
        uncertain = true;
        uncertain_live = true;
      }
      continue;
    }
    live_owners.extend(owner.map(|(owner, _)| *owner));
    match (name, cell) {
      (Some(_), Some(_)) => live = true,
      // The certificate binds a literal record directly but needs a cell binding to apply.
      (Some(_), None) if literal.is_some() => {}
      // A namespace read as a value escapes member resolution: the certificate refuses it.
      (None, _)
        if statements.iter().any(|statement| {
          statement.referenced_symbols.iter().any(|reference| {
            matches!(reference, SymbolOrMemberExprRef::Symbol(symbol) if symbol == *binding)
          })
        }) =>
      {
        evaluate = true;
      }
      // A declaration outside any cell is the canonical owner the certificate refuses.
      (Some(name), None) if declared_outside_cells(modules, donor, name) => evaluate = true,
      _ => {
        uncertain = true;
        uncertain_live = true;
      }
    }
  }
  if !live && !uncertain_live {
    evaluate = true;
  }

  // The certificate walks `Import` records only; any failure it meets is final, whatever it
  // meets first.
  let mut state = FxHashMap::default();
  let mut pending = vec![(donor.idx, false)];
  let mut edges = 0;
  while let Some((idx, leaving)) = pending.pop() {
    if leaving {
      state.insert(idx, 2_u8);
      continue;
    }
    match state.get(&idx) {
      Some(1) => {
        uncertain = true;
        continue;
      }
      Some(2) => continue,
      _ => {}
    }
    let Some(module) = modules[idx].as_normal() else {
      evaluate = true;
      continue;
    };
    if module.side_effects.has_side_effects()
      || module.meta.has_eval()
      || module.ast_usage.contains(EcmaModuleAstUsage::TopLevelAwait)
      || module.exports_kind == ExportsKind::CommonJs
    {
      evaluate = true;
    }
    if module.exports_kind != ExportsKind::Esm
      || is_json_module(module)
      || required.contains(&idx)
      || module.star_export_module_ids().next().is_some()
    {
      uncertain = true;
    }
    state.insert(idx, 1);
    pending.push((idx, true));
    for dependency in module.import_records.iter() {
      edges += 1;
      if dependency.kind != ImportKind::Import {
        evaluate = true;
        continue;
      }
      match dependency.resolved_module {
        Some(next) => pending.push((next, false)),
        None => evaluate = true,
      }
    }
    if state.len() > MAX_MODULES || edges > MAX_EDGES {
      uncertain = true;
      break;
    }
  }

  let verdict = if evaluate {
    Verdict::Evaluate
  } else if uncertain {
    Verdict::Uncertain
  } else {
    Verdict::Bypass
  };
  if verdict != Verdict::Bypass {
    bindings.clear();
    live_owners.clear();
  }
  Route {
    importer: importer.idx,
    record,
    donor: donor.idx,
    verdict,
    closure: Vec::new(),
    bindings,
    live_owners,
  }
}

/// Patch routing for one importer's [`Verdict::Bypass`] records: the bindings of the cells and
/// literal records that its record evaluates. A binding of another, which the importer doesn't
/// read, would load a module that no record ran.
pub fn bypass_bindings(
  routes: &[Route],
  importer: ModuleIdx,
) -> FxHashMap<ImportRecordIdx, FxHashMap<String, (ModuleIdx, String)>> {
  routes
    .iter()
    .filter(|route| route.importer == importer && route.verdict == Verdict::Bypass)
    .map(|route| {
      let bindings = route
        .bindings
        .iter()
        .filter(|(_, (owner, _))| route.live_owners.contains(owner))
        .map(|(name, binding)| (name.clone(), binding.clone()))
        .collect();
      (route.record, bindings)
    })
    .collect()
}

/// What one importer's [`Verdict::Bypass`] records evaluate in place of their donors, by record.
pub fn bypass_evaluations(
  routes: &[Route],
  importer: ModuleIdx,
) -> FxHashMap<ImportRecordIdx, &[ModuleIdx]> {
  routes
    .iter()
    .filter(|route| route.importer == importer && route.verdict == Verdict::Bypass)
    .map(|route| (route.record, route.live_owners.as_slice()))
    .collect()
}

/// One importer's [`Verdict::Uncertain`] records: the full build may have bound each straight to
/// cell records, without evaluating the donor's closure, or evaluated the donor.
pub fn uncertain_records(routes: &[Route], importer: ModuleIdx) -> FxHashSet<ImportRecordIdx> {
  routes
    .iter()
    .filter(|route| route.importer == importer && route.verdict == Verdict::Uncertain)
    .map(|route| route.record)
    .collect()
}

/// A stable description of every verdict, and of the cells and literal records that a bypass
/// evaluates: a live page holds the shape the full build chose, so a patch cannot change any of
/// them, nor load a module that no record ran. It leaves out an evaluation of a donor that
/// `resident` holds, which every page ran before the update ([`resident`]): its import loads the
/// page's instance, as an import of a module without cells does.
pub fn signature(
  modules: &IndexModules,
  routes: &[Route],
  resident: &FxHashSet<ModuleIdx>,
) -> Vec<String> {
  let id = |idx: ModuleIdx| modules[idx].stable_id().to_string();
  let mut signature = routes
    .iter()
    .filter(|route| route.verdict != Verdict::Evaluate || !resident.contains(&route.donor))
    .map(|route| {
      let mut bindings = route
        .bindings
        .iter()
        .map(|(name, (cell, cell_name))| format!("{name}={}:{cell_name}", id(*cell)))
        .collect::<Vec<_>>();
      bindings.sort_unstable();
      let closure = route.closure.iter().map(|idx| id(*idx)).collect::<Vec<_>>();
      let mut live_owners = route.live_owners.iter().map(|idx| id(*idx)).collect::<Vec<_>>();
      live_owners.sort_unstable();
      live_owners.dedup();
      format!(
        "{} -> {} {:?} [{}] ({}) <{}>",
        id(route.importer),
        id(route.donor),
        route.verdict,
        closure.join(","),
        bindings.join(","),
        live_owners.join(",")
      )
    })
    .collect::<Vec<_>>();
  signature.sort_unstable();
  signature
}

/// Modules whose edit could change an [`Verdict::Uncertain`] candidate: its importer, the
/// donor's closure, and every other importer of the donor.
pub fn uncertain_ends(modules: &IndexModules, routes: &[Route]) -> FxHashSet<ModuleIdx> {
  let mut ends = FxHashSet::default();
  let mut donors = FxHashSet::default();
  for route in routes.iter().filter(|route| route.verdict == Verdict::Uncertain) {
    ends.insert(route.importer);
    ends.extend(route.closure.iter().copied());
    donors.insert(route.donor);
  }
  if !donors.is_empty() {
    for module in modules.iter().filter_map(|module| module.as_normal()) {
      if module
        .import_records
        .iter()
        .any(|record| record.resolved_module.is_some_and(|idx| donors.contains(&idx)))
      {
        ends.insert(module.idx);
      }
    }
  }
  ends
}
