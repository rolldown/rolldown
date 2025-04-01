use std::{borrow::Cow, path::Path};

use anyhow::Ok;
use ast_visitors::DtsAstScanner;
use oxc::{
  ast_visit::Visit,
  codegen::{CodeGenerator, CodegenOptions},
  isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions},
  semantic::SemanticBuilder,
  span::SourceType,
};
use oxc_index::IndexVec;
use rolldown_common::{ModuleId, ModuleIdx, ModuleType, Output, SymbolRefDb, SymbolRefDbForModule};
use rolldown_ecmascript::EcmaCompiler;
use rolldown_plugin::{Plugin, PluginHookMeta, PluginOrder};
use rolldown_utils::dashmap::{FxDashMap, FxDashSet};
use sugar_path::SugarPath;
use types::{DtsChunk, DtsModule};

mod ast_visitors;
mod generate_stage;
mod link_stage;
mod types;

#[derive(Debug, Default)]
pub struct DtsPlugin {
  pub strip_internal: bool,
  pub dts_modules: FxDashMap<ModuleIdx, DtsModule>,
  pub symbols: FxDashMap<ModuleIdx, SymbolRefDbForModule>,
  pub module_id_to_module_idx: FxDashMap<ModuleId, ModuleIdx>,
  pub entries: FxDashSet<ModuleId>,
}

impl DtsPlugin {
  pub fn new(strip_internal: bool) -> Self {
    Self { strip_internal, ..Default::default() }
  }
}

impl Plugin for DtsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dts")
  }

  async fn transform_ast(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    self.module_id_to_module_idx.insert(args.id.into(), args.module_index);
    if matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) {
      let ret = args.ast.program.with_mut(|fields| {
        IsolatedDeclarations::new(
          fields.allocator,
          IsolatedDeclarationsOptions { strip_internal: self.strip_internal },
        )
        .build(fields.program)
      });

      // TODO BuildDiagnostic error
      if !ret.errors.is_empty() {
        return Err(anyhow::anyhow!("IsolatedDeclarations error"));
      }

      // Here using the ret.program to generate the d.ts file, and parse it to ast.
      // TODO here maybe could be using the isolated declarations ast directly, it need to reference the original ast at now.
      let codegen_ret =
        CodeGenerator::new().with_options(CodegenOptions::default()).build(&ret.program);

      let mut dts_ast = // TODO handle error
      EcmaCompiler::parse(args.id, codegen_ret.code, SourceType::ts()).expect("should success");

      let scoping =
        dts_ast.make_symbol_table_and_scope_tree_with_semantic_builder(SemanticBuilder::new());

      let module_id = ModuleId::new(args.id);

      let mut dts_module = {
        let mut dts_ast_scanner = DtsAstScanner::new(scoping, args.module_index, &module_id);

        dts_ast.program.with_mut(|fields| {
          dts_ast_scanner.visit_program(fields.program);
        });

        self.symbols.insert(args.module_index, dts_ast_scanner.symbol_ref_db);

        DtsModule {
          stable_id: args.stable_id.into(),
          module_index: args.module_index,
          module_id: module_id.clone(),
          named_imports: dts_ast_scanner.named_imports,
          named_exports: dts_ast_scanner.named_exports,
          stmt_infos: dts_ast_scanner.stmt_infos,
          import_records: dts_ast_scanner.import_records,
          default_export_ref: dts_ast_scanner.default_export_ref,
          namespace_object_ref: dts_ast_scanner.namespace_object_ref,
          has_star_exports: dts_ast_scanner.has_star_exports,
          dts_ast,
          import_record_to_module_id: IndexVec::new(),
          import_record_to_module_idx: IndexVec::new(),
        }
      };

      for import_record in &dts_module.import_records {
        let resolved_id = ctx.resolve(&import_record.module_request, Some(args.id), None).await??;
        ctx.load(&resolved_id.id, None, None).await?;
        dts_module.import_record_to_module_id.push(resolved_id.id.into());
      }

      self.dts_modules.insert(args.module_index, dts_module);

      if args.is_user_defined_entry {
        self.entries.insert(module_id);
      }
    }
    Ok(args.ast)
  }

  // The rolldown strip types at the end of the build process, make sure to run this plugin before that.
  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Post) })
  }

  async fn render_start(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookRenderStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    let mut dts_modules = IndexVec::with_capacity(self.module_id_to_module_idx.len());
    for module_ref in &self.dts_modules {
      if let Some((module_idx, mut dts_module)) = self.dts_modules.remove(module_ref.key()) {
        for module_id in &dts_module.import_record_to_module_id {
          dts_module
            .import_record_to_module_idx
            .push(*self.module_id_to_module_idx.get(module_id).unwrap().value());
        }
        dts_modules.insert(module_idx, Some(dts_module));
      } else {
        dts_modules.insert(*module_ref.key(), None);
      }
    }

    let mut symbols = IndexVec::with_capacity(self.module_id_to_module_idx.len());
    for symbol_ref in &self.symbols {
      if let Some((module_idx, symbol_db)) = self.symbols.remove(symbol_ref.key()) {
        symbols.insert(module_idx, Some(symbol_db));
      } else {
        symbols.insert(*symbol_ref.key(), None);
      }
    }

    let entries = self
      .entries
      .iter()
      .map(|module_id| *self.module_id_to_module_idx.get(&module_id).unwrap().value())
      .collect::<Vec<_>>();

    let link_stage = link_stage::DtsLinkStage::new(dts_modules, entries, SymbolRefDb::new(symbols));
    link_stage.link();

    // TODO emit warnings and errors
    Ok(())
  }

  async fn generate_bundle(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    for output in args.bundle.iter() {
      if let Output::Chunk(chunk) = output {
        let dts_modules = chunk
          .modules
          .keys
          .iter()
          .filter_map(|module_id| self.module_id_to_module_idx.get(module_id).map(|s| *s.value()))
          .collect::<Vec<_>>();
        let chunk = DtsChunk {
          dts_modules,
          name: {
            let mut name = Path::new(chunk.name.as_str()).to_path_buf();
            name.set_extension("d.ts");
            name.to_slash_lossy().into()
          },
          is_entry: chunk.is_entry,
        };
      }
    }
    Ok(())
  }
}
