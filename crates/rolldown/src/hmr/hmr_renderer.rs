use oxc_traverse::traverse_mut;
use rolldown_common::{Module, ModuleIdx};
use rolldown_ecmascript::{EcmaAst, EcmaCompiler, PrintCommentsOptions, PrintOptions};
use rolldown_ecmascript_utils::AstSnippet;
use rolldown_fs::FileSystem;
use rolldown_sourcemap::{Source, SourceMapSource};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::{
  concat_string,
  indexmap::{FxIndexMap, FxIndexSet},
  rayon::{IntoParallelIterator, ParallelIterator},
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{
  hmr_ast_finalizer::{HmrAstFinalizer, ModuleInitializerMode},
  hmr_stage::HmrStage,
};

struct ModuleRenderInput {
  idx: ModuleIdx,
  ecma_ast: EcmaAst,
}

type RenderedModule = [Box<dyn Source + Send>; 3];

pub(super) struct RenderedModules {
  pub(super) init_fn_names: FxHashMap<ModuleIdx, String>,
  pub(super) sources: Vec<Box<dyn Source + Send>>,
}

impl<Fs: FileSystem + Clone + 'static> HmrStage<'_, Fs> {
  pub(super) fn render_modules(
    &self,
    modules_to_render: &FxIndexSet<ModuleIdx>,
    initializer_mode: impl Fn(ModuleIdx) -> ModuleInitializerMode + Sync,
  ) -> RenderedModules {
    let module_idx_to_init_fn_name = modules_to_render
      .iter()
      .enumerate()
      .map(|(index, module_idx)| {
        let Module::Normal(module) = &self.module_table().modules[*module_idx] else {
          unreachable!(
            "External modules should be removed before. But got {:?}",
            self.module_table().modules[*module_idx].id().as_str()
          );
        };
        let prefix = if module.exports_kind.is_commonjs() { "require" } else { "init" };

        // We use `index` as a part of the function name to avoid name collision without needing to deconflict.
        (*module_idx, format!("{}_{}_{}", prefix, module.repr_name, index))
      })
      .collect::<FxHashMap<_, _>>();

    let index_ecma_ast = self.index_ecma_ast();
    let module_render_inputs = modules_to_render
      .iter()
      .copied()
      .map(|module_idx| {
        let Module::Normal(module) = &self.module_table().modules[module_idx] else {
          unreachable!("Only normal modules should be rendered");
        };

        debug_assert_eq!(module_idx, module.idx);
        let ecma_ast =
          index_ecma_ast[module_idx].as_ref().expect("Normal module should have an AST");

        ModuleRenderInput { idx: module.idx, ecma_ast: ecma_ast.clone_with_another_arena() }
      })
      .collect::<Vec<_>>();

    let sources = module_render_inputs
      .into_par_iter()
      .enumerate()
      .flat_map(|(index, render_input)| {
        let module_initializer_mode = initializer_mode(render_input.idx);
        self.render_module(
          render_input,
          index,
          &module_idx_to_init_fn_name,
          module_initializer_mode,
        )
      })
      .collect::<Vec<_>>();

    RenderedModules { init_fn_names: module_idx_to_init_fn_name, sources }
  }

  fn render_module(
    &self,
    render_input: ModuleRenderInput,
    index: usize,
    module_idx_to_init_fn_name: &FxHashMap<ModuleIdx, String>,
    module_initializer_mode: ModuleInitializerMode,
  ) -> RenderedModule {
    let ModuleRenderInput { idx: module_idx, ecma_ast: mut ast } = render_input;

    let Module::Normal(module) = &self.module_table().modules[module_idx] else {
      unreachable!("Only normal modules should be rendered");
    };

    let enable_sourcemap = self.options.sourcemap.is_some() && !module.is_virtual();
    let use_pife_for_module_wrappers =
      self.options.optimization.is_pife_for_module_wrappers_enabled();
    let modules = &self.module_table().modules;
    ast.program.with_mut(|fields| {
      let scoping = EcmaAst::make_semantic(fields.program, /*with_cfg*/ false).into_scoping();

      let mut finalizer = HmrAstFinalizer {
        modules,
        alloc: fields.allocator,
        snippet: AstSnippet::new(fields.allocator),
        builder: &oxc::ast::AstBuilder::new(fields.allocator),
        import_bindings: FxHashMap::default(),
        module,
        exports: oxc::allocator::Vec::new_in(fields.allocator),
        affected_module_idx_to_init_fn_name: module_idx_to_init_fn_name,
        use_pife_for_module_wrappers,
        module_initializer_mode,
        dependencies: FxIndexSet::default(),
        imports: FxHashSet::default(),
        generated_static_import_infos: FxHashMap::default(),
        re_export_all_dependencies: FxIndexSet::default(),
        generated_static_import_stmts_from_external: FxIndexMap::default(),
        unique_index: index,
        named_exports: FxHashMap::default(),
      };

      traverse_mut(&mut finalizer, fields.allocator, fields.program, scoping, ());
    });

    let codegen = EcmaCompiler::print_with(
      &ast,
      PrintOptions {
        sourcemap: enable_sourcemap,
        filename: module.id.to_string(),
        comments: PrintCommentsOptions {
          legal: false,
          annotation: self.options.comments.annotation,
          jsdoc: self.options.comments.jsdoc,
        },
        initial_indent: 0,
      },
    );

    let intro_comment: Box<dyn Source + Send> =
      Box::new(concat_string!("//#region ", module.debug_id));
    let outro_comment: Box<dyn Source + Send> = Box::new(concat_string!("//#endregion"));

    let code_source: Box<dyn Source + Send> = if let Some(map) = codegen.map {
      Box::new(SourceMapSource::new(codegen.code, map.into_inner()))
    } else {
      Box::new(codegen.code)
    };

    [intro_comment, code_source, outro_comment]
  }
}
