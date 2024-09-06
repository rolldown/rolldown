use std::{
  path::Path,
  time::{SystemTime, UNIX_EPOCH},
};

use rolldown_common::{
  FileNameRenderOptions, FilenameTemplate, NormalizedBundlerOptions, Output, OutputAsset,
  SourceMapType,
};
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};

use crate::{
  module_loader::hmr_module_loader::HmrModuleLoaderOutput,
  utils::render_ecma_module::render_ecma_module, BundleOutput,
};

pub fn render_hmr_chunk(
  options: &NormalizedBundlerOptions,
  hmr_module_loader_output: &mut HmrModuleLoaderOutput,
) -> BundleOutput {
  let module_sources = hmr_module_loader_output
    .diff_modules
    .par_iter()
    .filter_map(|id| hmr_module_loader_output.module_table.modules[*id].as_ecma())
    .map(|m| {
      (
        m.idx,
        m.id.clone(),
        render_ecma_module(
          m,
          &hmr_module_loader_output.index_ecma_ast[m.ecma_ast_idx()].0,
          m.id.as_ref(),
          options,
        ),
      )
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
    let map_filename = format!("{filename}.map",);
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
          Path::new(&map_filename).file_name().expect("should have filename").to_string_lossy()
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
    filename,
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
