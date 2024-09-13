use std::{
  path::Path,
  time::{SystemTime, UNIX_EPOCH},
};

use arcstr::ArcStr;
use oxc::ast::VisitMut;
use rolldown_common::{
  FileNameRenderOptions, FilenameTemplate, Module, NormalizedBundlerOptions, Output, OutputAsset,
  SourceMapType,
};
use rolldown_ecmascript::{AstSnippet, EcmaCompiler};
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelBridge, ParallelIterator};
use rustc_hash::FxHashSet;

use crate::{
  module_finalizers::isolating::{IsolatingModuleFinalizer, IsolatingModuleFinalizerContext},
  module_loader::hmr_module_loader::HmrModuleLoaderOutput,
  types::symbols::Symbols,
  utils::render_ecma_module::render_ecma_module,
  BundleOutput,
};

#[allow(clippy::too_many_lines)]
pub fn render_hmr_chunk(
  options: &NormalizedBundlerOptions,
  hmr_module_loader_output: &mut HmrModuleLoaderOutput,
) -> BundleOutput {
  hmr_module_loader_output
    .index_ecma_ast
    .iter_mut()
    .par_bridge()
    .filter(|(_ast, owner)| {
      hmr_module_loader_output.module_table.modules[*owner].as_ecma().is_some()
        && hmr_module_loader_output.diff_modules.contains(owner)
    })
    .for_each(|(ast, owner)| {
      let Module::Ecma(module) = &hmr_module_loader_output.module_table.modules[*owner] else {
        return;
      };
      ast.program.with_mut(|fields| {
        let (oxc_program, alloc) = (fields.program, fields.allocator);
        let mut finalizer = IsolatingModuleFinalizer {
          alloc,
          scope: &module.scope,
          ctx: &IsolatingModuleFinalizerContext {
            module,
            modules: &hmr_module_loader_output.module_table.modules,
            symbols: &Symbols::default(), // TODO
          },
          snippet: AstSnippet::new(alloc),
          generated_imports_set: FxHashSet::default(),
          generated_imports: oxc::allocator::Vec::new_in(alloc),
          generated_exports: oxc::allocator::Vec::new_in(alloc),
        };
        finalizer.visit_program(oxc_program);
      });
    });

  let module_sources = hmr_module_loader_output
    .diff_modules
    .par_iter()
    .copied()
    .filter_map(|id| hmr_module_loader_output.module_table.modules[id].as_ecma())
    .map(|module| {
      let enable_sourcemap = !options.sourcemap.is_hidden() && !module.is_virtual();
      let render_output = EcmaCompiler::print(
        &hmr_module_loader_output.index_ecma_ast[module.ecma_ast_idx()].0,
        &module.id,
        enable_sourcemap,
      );
      (module.idx, module.id.clone(), render_ecma_module(module, options, render_output))
    })
    .collect::<Vec<_>>();

  let mut concat_source = ConcatSource::default();

  concat_source.add_source(Box::new(RawSource::new(format!(
    "self.rolldown_runtime.patch([{}], function(){{\n",
    hmr_module_loader_output
      .changed_modules
      .iter()
      .map(|idx| format!("'{}'", hmr_module_loader_output.module_table.modules[*idx].stable_id()))
      .collect::<Vec<_>>()
      .join(", ")
  ))));

  module_sources.into_iter().for_each(|(module_idx, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      concat_source.add_source(Box::new(RawSource::new(format!(
        "rolldown_runtime.define('{}',function(require, module, exports){{\n",
        hmr_module_loader_output.module_table.modules[module_idx].stable_id()
      ))));
      for source in emitted_sources {
        concat_source.add_source(source);
      }
      concat_source.add_source(Box::new(RawSource::new("});".to_string())));
    }
  });

  concat_source.add_source(Box::new(RawSource::new("});".to_string())));

  let (mut content, map) = concat_source.content_and_sourcemap();

  let mut assets = vec![];

  let filename =
    FilenameTemplate::new("hmr-update.[hash].js".into()).render(&FileNameRenderOptions {
      hash: Some(
        &SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .expect("should have time")
          .as_millis()
          .to_string(),
      ),
      ..Default::default()
    });

  if let Some(map) = map {
    let map_filename: ArcStr = format!("{filename}.map",).into();
    match options.sourcemap {
      SourceMapType::File => {
        let source = map.to_json_string();
        assets.push(Output::Asset(Box::new(OutputAsset {
          filename: map_filename.clone(),
          source: source.into(),
          original_file_name: None,
          name: None,
        })));
        content.push_str(&format!(
          "\n//# sourceMappingURL={}",
          Path::new(map_filename.as_str())
            .file_name()
            .expect("should have filename")
            .to_string_lossy()
        ));
      }
      SourceMapType::Inline => {
        let data_url = map.to_data_url();
        content.push_str(&format!("\n//# sourceMappingURL={data_url}"));
      }
      SourceMapType::Hidden => {}
    }
  }

  assets.push(Output::Asset(Box::new(OutputAsset {
    filename: filename.into(),
    source: content.into(),
    original_file_name: None,
    name: None,
  })));

  BundleOutput {
    warnings: std::mem::take(&mut hmr_module_loader_output.warnings),
    errors: vec![],
    assets,
  }
}
