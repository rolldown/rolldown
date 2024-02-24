use std::{fmt::Debug, sync::Arc};

use index_vec::IndexVec;
use oxc::{
  ast::VisitMut,
  semantic::SymbolId,
  span::{Atom, Span},
};
use rolldown_common::{
  DebugStmtInfoForTreeShaking, ExportsKind, ImportRecord, ImportRecordId, LocalExport, ModuleType,
  NamedImport, NormalModuleId, ResolvedExport, ResourceId, StmtInfo, StmtInfos, SymbolRef,
};
use rolldown_oxc::{AstSnippet, OxcCompiler, OxcProgram};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::bundler::{
  finalizer::{Finalizer, FinalizerContext},
  types::{ast_scope::AstScope, linking_metadata::LinkingMetadataVec},
};

use super::{Module, ModuleRenderContext, ModuleVec};

#[derive(Debug)]
pub struct NormalModule {
  pub exec_order: u32,
  pub source: Arc<str>,
  pub id: NormalModuleId,
  pub is_user_defined_entry: bool,
  pub resource_id: ResourceId,
  pub pretty_path: String,
  /// Representative name of `FilePath`, which is created by `FilePath#representative_name` belong to `resource_id`
  pub repr_name: String,
  pub module_type: ModuleType,
  pub namespace_symbol: SymbolRef,
  pub named_imports: FxHashMap<SymbolId, NamedImport>,
  pub named_exports: FxHashMap<Atom, LocalExport>,
  /// `stmt_infos[0]` represents the namespace binding statement
  pub stmt_infos: StmtInfos,
  pub import_records: IndexVec<ImportRecordId, ImportRecord>,
  /// The key is the `Span` of `ImportDeclaration`, `ImportExpression`, `ExportNamedDeclaration`, `ExportAllDeclaration`
  /// and `CallExpression`(only when the callee is `require`).
  pub imports: FxHashMap<Span, ImportRecordId>,
  // [[StarExportEntries]] in https://tc39.es/ecma262/#sec-source-text-module-records
  pub star_exports: Vec<ImportRecordId>,
  pub exports_kind: ExportsKind,
  pub scope: AstScope,
  pub default_export_ref: SymbolRef,
  pub sourcemap_chain: Vec<rolldown_sourcemap::SourceMap>,
  pub is_included: bool,
}

impl NormalModule {
  pub fn finalize(&self, ctx: FinalizerContext<'_>, ast: &mut OxcProgram) {
    let (oxc_program, alloc) = ast.program_mut_and_allocator();

    let mut finalizer =
      Finalizer { alloc, ctx, scope: &self.scope, snippet: &AstSnippet::new(alloc) };

    finalizer.visit_program(oxc_program);
  }

  #[allow(clippy::unnecessary_wraps)]
  pub fn render(
    &self,
    _ctx: &ModuleRenderContext<'_>,
    ast: &OxcProgram,
  ) -> Option<MagicString<'static>> {
    let generated_code = OxcCompiler::print(ast);
    let mut source = MagicString::new(generated_code);

    source.prepend(format!("// {}\n", self.pretty_path));

    Some(source)
  }

  pub(crate) fn add_exports_for_export_star(
    &self,
    id: NormalModuleId,
    metas: &mut LinkingMetadataVec,
    modules: &ModuleVec,
    module_stack: &mut Vec<NormalModuleId>,
  ) {
    if module_stack.contains(&self.id) {
      return;
    }
    module_stack.push(self.id);

    self.star_export_modules().for_each(|importee_id| {
      let importee = &modules[importee_id];
      match importee {
        Module::External(_) => {
          // This will be resolved at run time instead
        }
        Module::Normal(importee) => {
          // Export star from commonjs will be resolved at runtime
          if importee.exports_kind == ExportsKind::CommonJs {
            return;
          }

          importee.named_exports.iter().for_each(|(alias, importee_export)| {
            // ES6 export star ignore default export
            if alias == &"default" {
              return;
            }

            // This export star is shadowed if any file in the stack has a matching real named export
            if module_stack
              .iter()
              .copied()
              .filter_map(|id| modules[id].as_normal())
              .any(|prev_module| prev_module.named_exports.contains_key(alias))
            {
              return;
            }

            let importer_meta = &mut metas[id];

            importer_meta
              .resolved_exports
              .entry(alias.clone())
              .and_modify(|existing| {
                if existing.symbol_ref != importee_export.referenced {
                  // This means that the importer already has a export with the same name, and it's not from its own
                  // local named exports. Such a situation is already handled above, so this is a case of ambiguity.
                  existing
                    .potentially_ambiguous_symbol_refs
                    .get_or_insert_with(Default::default)
                    .push(importee_export.referenced);
                }
              })
              .or_insert_with(|| ResolvedExport::new(importee_export.referenced));
          });

          importee.add_exports_for_export_star(id, metas, modules, module_stack);
        }
      }
    });

    module_stack.remove(module_stack.len() - 1);
  }

  pub fn star_export_modules(&self) -> impl Iterator<Item = NormalModuleId> + '_ {
    self.star_exports.iter().map(|rec_id| {
      let rec = &self.import_records[*rec_id];
      rec.resolved_module
    })
  }

  pub fn _importee_id_by_span(&self, span: Span) -> NormalModuleId {
    let record = &self.import_records[self.imports[&span]];
    record.resolved_module
  }

  pub fn to_debug_normal_module_for_tree_shaking(&self) -> DebugNormalModuleForTreeShaking {
    DebugNormalModuleForTreeShaking {
      id: self.repr_name.to_string(),
      is_included: self.is_included,
      stmt_infos: self
        .stmt_infos
        .iter()
        .map(StmtInfo::to_debug_stmt_info_for_tree_shaking)
        .collect(),
    }
  }
}

#[derive(Debug)]
pub struct DebugNormalModuleForTreeShaking {
  pub id: String,
  pub is_included: bool,
  pub stmt_infos: Vec<DebugStmtInfoForTreeShaking>,
}
