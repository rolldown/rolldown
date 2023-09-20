use std::{cell::Cell, fmt::Debug};

use index_vec::IndexVec;
use oxc::{
  ast::VisitMut,
  semantic::{ReferenceId, ScopeTree, SymbolId},
  span::Atom,
};
use rolldown_common::{
  ImportRecord, ImportRecordId, LocalOrReExport, ModuleId, NamedImport, ResolvedExport, ResourceId,
  StmtInfo, StmtInfoId, SymbolRef,
};
use rolldown_oxc::{DummyIn, IntoIn, OxcProgram};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{module::ModuleFinalizeContext, module_id::ModuleVec};
use crate::bundler::{
  graph::symbols::Symbols,
  module::module::Module,
  visitors::{FinalizeContext, Finalizer},
};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub id: ModuleId,
  pub resource_id: ResourceId,
  pub ast: OxcProgram,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalOrReExport>,
  pub stmt_infos: IndexVec<StmtInfoId, StmtInfo>,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  pub external_requests: FxHashSet<Atom>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  // resolved
  pub resolved_exports: FxHashMap<Atom, ResolvedExport>,
  pub resolved_star_exports: Vec<ModuleId>,
  pub scope: ScopeTree,
  pub default_export_symbol: Option<SymbolId>,
  pub namespace_symbol: (SymbolRef, ReferenceId),
  pub is_symbol_for_namespace_referenced: bool,
}

pub enum Resolution {
  None,
  Ambiguous,
  Found(SymbolRef),
}

impl NormalModule {
  pub fn finalize(&mut self, ctx: ModuleFinalizeContext) {
    let (program, allocator) = self.ast.program_mut_and_allocator();
    let mut finalizer = Finalizer::new(FinalizeContext {
      allocator,
      symbols: ctx.symbols,
      scope: &self.scope,
      id: self.id,
      default_export_symbol: self.default_export_symbol,
      final_names: ctx.canonical_names,
      external_requests: &self.external_requests,
    });
    finalizer.visit_program(program);
  }

  pub fn initialize_namespace(&mut self) {
    use oxc::allocator::Vec;
    use oxc::ast::ast;
    let (program, alloc) = self.ast.program_mut_and_allocator();
    let mut properties = Vec::new_in(alloc);
    self
      .resolved_exports
      .iter()
      .for_each(|(exported_name, info)| {
        let mut statements = Vec::new_in(alloc);

        statements.push(ast::Statement::ReturnStatement(
          ast::ReturnStatement {
            span: Default::default(),
            argument: Some(ast::Expression::Identifier(
              ast::IdentifierReference {
                span: Default::default(),
                name: Atom::new_inline("$REPLACE$"),
                reference_id: Cell::new(Some(info.local_ref)),
              }
              .into_in(alloc),
            )),
          }
          .into_in(alloc),
        ));

        properties.push(ast::ObjectPropertyKind::ObjectProperty(
          ast::ObjectProperty {
            span: Default::default(),
            kind: ast::PropertyKind::Get,
            key: ast::PropertyKey::Identifier(
              ast::IdentifierName {
                span: Default::default(),
                name: exported_name.clone(),
              }
              .into_in(alloc),
            ),
            value: ast::Expression::FunctionExpression(
              ast::Function {
                r#type: ast::FunctionType::FunctionExpression,
                span: Default::default(),
                id: None,
                expression: false,
                generator: false,
                r#async: false,
                params: DummyIn::dummy_in(alloc),
                body: Some(
                  ast::FunctionBody {
                    span: Default::default(),
                    directives: DummyIn::dummy_in(alloc),
                    statements,
                  }
                  .into_in(alloc),
                ),
                type_parameters: None,
                return_type: None,
                modifiers: Default::default(),
              }
              .into_in(alloc),
            ),
            init: None,
            method: false,
            shorthand: false,
            computed: false,
          }
          .into_in(alloc),
        ))
      });
    let exp = ast::Expression::ObjectExpression(
      ast::ObjectExpression {
        span: Default::default(),
        properties,
        trailing_comma: None,
      }
      .into_in(alloc),
    );

    let mut declarations = Vec::new_in(alloc);
    declarations.push(ast::VariableDeclarator {
      span: Default::default(),
      kind: ast::VariableDeclarationKind::Var,
      id: ast::BindingPattern {
        kind: ast::BindingPatternKind::BindingIdentifier(
          ast::BindingIdentifier {
            span: Default::default(),
            name: Atom::new_inline(""),
            symbol_id: Cell::new(Some(self.namespace_symbol.0.symbol)),
          }
          .into_in(alloc),
        ),
        type_annotation: None,
        optional: false,
      },
      init: Some(exp),
      definite: false,
    });
    let stmt = ast::Statement::Declaration(ast::Declaration::VariableDeclaration(
      ast::VariableDeclaration {
        declarations,
        ..DummyIn::dummy_in(alloc)
      }
      .into_in(alloc),
    ));
    let idx = program.body.len();
    program.body.push(stmt);
    self.stmt_infos.push(StmtInfo {
      stmt_idx: idx,
      declared_symbols: vec![self.namespace_symbol.0.symbol],
    });
  }

  // https://tc39.es/ecma262/#sec-getexportednames
  pub fn get_exported_names<'modules>(
    &'modules self,
    stack: &mut Vec<ModuleId>,
    modules: &'modules ModuleVec,
  ) -> FxHashSet<&'modules Atom> {
    if stack.contains(&self.id) {
      // cycle
      return Default::default();
    }

    stack.push(self.id);

    let ret: FxHashSet<&'modules Atom> = {
      self
        .star_exports
        .iter()
        .copied()
        .map(|id| &self.import_records[id])
        .flat_map(|rec| {
          debug_assert!(rec.resolved_module.is_valid());
          let importee = &modules[rec.resolved_module];
          match importee {
            Module::Normal(importee) => importee
              .get_exported_names(stack, modules)
              .into_iter()
              .filter(|name| name.as_str() != "default"),
            Module::External(_) => {
              unimplemented!("handle external module")
            }
          }
        })
        .chain(self.named_exports.keys())
        .collect()
    };

    stack.pop();
    ret
  }

  // https://tc39.es/ecma262/#sec-resolveexport
  pub fn resolve_export<'modules>(
    &'modules self,
    export_name: &'modules Atom,
    resolve_set: &mut Vec<(ModuleId, &'modules Atom)>,
    modules: &'modules ModuleVec,
    symbols: &mut Symbols,
  ) -> Resolution {
    let record = (self.id, export_name);
    if resolve_set.iter().rev().any(|prev| prev == &record) {
      unimplemented!("handle cycle")
    }
    resolve_set.push(record);

    let ret = if let Some(info) = self.named_exports.get(export_name) {
      match info {
        LocalOrReExport::Local(local) => {
          if let Some(named_import) = self.named_imports.get(&local.referenced.symbol) {
            let record = &self.import_records[named_import.record_id];
            let importee = &modules[record.resolved_module];
            match importee {
              Module::Normal(importee) => {
                let resolved = if named_import.is_imported_star {
                  Resolution::Found(importee.namespace_symbol.0)
                } else {
                  importee.resolve_export(&named_import.imported, resolve_set, modules, symbols)
                };
                if let Resolution::Found(exist) = &resolved {
                  symbols.union(local.referenced, *exist)
                }
                resolved
              }
              Module::External(_) => {
                unimplemented!("handle external module")
              }
            }
          } else {
            Resolution::Found(local.referenced)
          }
        }
        LocalOrReExport::Re(re) => {
          let record = &self.import_records[re.record_id];
          let importee = &modules[record.resolved_module];
          match importee {
            Module::Normal(importee) => {
              if re.is_imported_star {
                return Resolution::Found(importee.namespace_symbol.0);
              } else {
                importee.resolve_export(&re.imported, resolve_set, modules, symbols)
              }
            }
            Module::External(_) => {
              unimplemented!("handle external module")
            }
          }
        }
      }
    } else {
      if export_name.as_str() == "default" {
        return Resolution::None;
      }
      let mut star_resolution: Option<SymbolRef> = None;
      for e in &self.star_exports {
        let rec = &self.import_records[*e];
        let importee = &modules[rec.resolved_module];
        match importee {
          Module::Normal(importee) => {
            match importee.resolve_export(export_name, resolve_set, modules, symbols) {
              Resolution::None => continue,
              Resolution::Ambiguous => return Resolution::Ambiguous,
              Resolution::Found(exist) => {
                if let Some(star_resolution) = star_resolution {
                  if star_resolution == exist {
                    continue;
                  } else {
                    return Resolution::Ambiguous;
                  }
                } else {
                  star_resolution = Some(exist)
                }
              }
            }
          }
          Module::External(_) => {
            unimplemented!("handle external module")
          }
        }
      }

      star_resolution
        .map(Resolution::Found)
        .unwrap_or(Resolution::None)
    };

    resolve_set.pop();

    ret
  }

  pub fn resolve_star_exports(&self, modules: &ModuleVec) -> Vec<ModuleId> {
    let mut visited = FxHashSet::default();
    let mut resolved = vec![];
    let mut queue = self
      .star_exports
      .iter()
      .map(|rec_id| {
        let rec = &self.import_records[*rec_id];
        rec.resolved_module
      })
      .collect::<Vec<_>>();

    while let Some(importee_id) = queue.pop() {
      if !visited.contains(&importee_id) {
        visited.insert(importee_id);
        resolved.push(importee_id);
        let importee = &modules[importee_id];
        match importee {
          Module::Normal(importee) => queue.extend(
            importee
              .star_exports
              .iter()
              .map(|rec_id| importee.import_records[*rec_id].resolved_module),
          ),
          Module::External(_) => todo!(),
        }
      }
    }

    resolved
  }
}
