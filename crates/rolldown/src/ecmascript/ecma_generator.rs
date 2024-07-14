use crate::{
  types::generator::{GenerateContext, Generator},
  utils::{
    chunk::{
      generate_rendered_chunk, render_chunk_exports::render_chunk_exports,
      render_chunk_imports::render_chunk_imports,
    },
    render_ecma_module::render_ecma_module,
  },
};

use anyhow::Result;
use rolldown_common::{
  AssetMeta, ChunkKind, EcmaAssetMeta, ExportsKind, OutputFormat, PreliminaryAsset, RenderedModule,
  WrapKind,
};
use rolldown_sourcemap::{ConcatSource, RawSource};
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;
use sugar_path::SugarPath;

pub struct EcmaGenerator;

impl Generator for EcmaGenerator {
  #[allow(clippy::too_many_lines)]
  async fn render_preliminary_assets<'a>(
    ctx: &GenerateContext<'a>,
  ) -> Result<Vec<PreliminaryAsset>> {
    let mut rendered_modules = FxHashMap::default();
    let mut concat_source = ConcatSource::default();

    let rendered_chunk = match ctx.options.format {
      OutputFormat::Esm | OutputFormat::Cjs | OutputFormat::Iife => {
        let mut rendered_iter = ctx
          .chunk
          .modules
          .par_iter()
          .copied()
          .filter_map(|id| ctx.link_output.module_table.modules[id].as_ecma())
          .map(|m| {
            (
              m.idx,
              &m.resource_id,
              render_ecma_module(
                m,
                &ctx.link_output.ast_table[m.idx],
                m.resource_id.as_ref(),
                ctx.options,
              ),
            )
          })
          .collect::<Vec<_>>()
          .into_iter()
          .peekable();

        let maybe_runtime_module = rendered_iter.peek();

        match maybe_runtime_module {
          Some((id, _, _))
            if *id == ctx.link_output.runtime.id()
              && matches!(ctx.options.format, OutputFormat::Cjs) =>
          {
            let maybe_runtime_module = rendered_iter.next();
            if let Some((_, _module_resource_id, Some(emitted_sources))) = maybe_runtime_module {
              for source in emitted_sources {
                concat_source.add_source(source);
              }
            }
            concat_source.add_source(Box::new(RawSource::new(render_chunk_imports(
              ctx.chunk,
              ctx.link_output,
              ctx.chunk_graph,
              ctx.options,
            ))));
          }
          _ => {
            concat_source.add_source(Box::new(RawSource::new(render_chunk_imports(
              ctx.chunk,
              ctx.link_output,
              ctx.chunk_graph,
              ctx.options,
            ))));
          }
        }

        // TODO indent chunk content for iife format
        if matches!(ctx.options.format, OutputFormat::Iife) {
          concat_source.add_source(Box::new(RawSource::new("(function() {\n".to_string())));
        }

        rendered_iter.for_each(|(_id, module_resource_id, module_render_output)| {
          if let Some(emitted_sources) = module_render_output {
            for source in emitted_sources {
              concat_source.add_source(source);
            }
          }

          // FIXME: NAPI-RS used CStr under the hood, so it can't handle null byte in the string.
          if !module_resource_id.starts_with('\0') {
            rendered_modules.insert(module_resource_id.clone(), RenderedModule { code: None });
          }
        });

        if matches!(ctx.options.format, OutputFormat::Iife) {
          concat_source.add_source(Box::new(RawSource::new("})();".to_string())));
        }

        generate_rendered_chunk(
          ctx.chunk,
          ctx.link_output,
          ctx.options,
          rendered_modules,
          ctx.chunk_graph,
        )
      }
      OutputFormat::App => {
        ctx
          .chunk
          .modules
          .par_iter()
          .copied()
          .filter_map(|id| ctx.link_output.module_table.modules[id].as_ecma())
          .filter_map(|m| {
            render_ecma_module(
              m,
              &ctx.link_output.ast_table[m.idx],
              m.resource_id.as_ref(),
              ctx.options,
            )
            .map(|rendered| (&m.resource_id, rendered))
          })
          .collect::<Vec<_>>()
          .into_iter()
          .for_each(|(module_resource_id, module_render_output)| {
            let emitted_sources = module_render_output;
            for source in emitted_sources {
              concat_source.add_source(source);
            }

            // FIXME: NAPI-RS used CStr under the hood, so it can't handle null byte in the string.
            if !module_resource_id.starts_with('\0') {
              rendered_modules.insert(module_resource_id.clone(), RenderedModule { code: None });
            }
          });
        generate_rendered_chunk(
          ctx.chunk,
          ctx.link_output,
          ctx.options,
          rendered_modules,
          ctx.chunk_graph,
        )
      }
    };

    // add banner
    if let Some(banner) = ctx.options.banner.as_ref() {
      if let Some(banner_txt) = banner.call(&rendered_chunk).await? {
        if !banner_txt.is_empty() {
          concat_source.add_prepend_source(Box::new(RawSource::new(banner_txt)));
        }
      }
    }

    // Add `use strict` directive if needed. This must come before the banner, because users might use banner to add hashbang.
    if matches!(ctx.options.format, OutputFormat::Cjs) {
      let are_modules_all_strict = ctx
        .chunk
        .modules
        .iter()
        .filter_map(|id| ctx.link_output.module_table.modules[*id].as_ecma())
        .all(|ecma_module| {
          let is_esm = matches!(&ecma_module.exports_kind, ExportsKind::Esm);
          is_esm || ctx.link_output.ast_table[ecma_module.idx].contains_use_strict
        });

      if are_modules_all_strict {
        concat_source.add_prepend_source(Box::new(RawSource::new("\"use strict\";\n".to_string())));
      }
    }

    if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
      // let entry = &ctx.link_output.module_table.normal_modules[entry_id];
      let entry_meta = &ctx.link_output.metas[entry_id];
      match ctx.options.format {
        OutputFormat::Esm => match entry_meta.wrap_kind {
          WrapKind::Esm => {
            // init_xxx()
            let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
            let wrapper_ref_name =
              ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
            concat_source.add_source(Box::new(RawSource::new(format!("{wrapper_ref_name}();",))));
          }
          WrapKind::Cjs => {
            // "export default require_xxx();"
            let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
            let wrapper_ref_name =
              ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
            concat_source.add_source(Box::new(RawSource::new(format!(
              "export default {wrapper_ref_name}();\n"
            ))));
          }
          WrapKind::None => {}
        },
        OutputFormat::Cjs | OutputFormat::App | OutputFormat::Iife => {}
      }
    }

    if let Some(exports) =
      render_chunk_exports(ctx.chunk, &ctx.link_output.runtime, ctx.link_output, ctx.options)
    {
      concat_source.add_source(Box::new(RawSource::new(exports)));
    }

    // add footer
    if let Some(footer) = ctx.options.footer.as_ref() {
      if let Some(footer_txt) = footer.call(&rendered_chunk).await? {
        if !footer_txt.is_empty() {
          concat_source.add_source(Box::new(RawSource::new(footer_txt)));
        }
      }
    }

    let (content, mut map) = concat_source.content_and_sourcemap();

    // Here file path is generated by chunk file name template, it maybe including path segments.
    // So here need to read it's parent directory as file_dir.
    let file_path = ctx.options.cwd.as_path().join(&ctx.options.dir).join(
      ctx
        .chunk
        .preliminary_filename
        .as_deref()
        .expect("chunk file name should be generated before rendering")
        .as_str(),
    );
    let file_dir = file_path.parent().expect("chunk file name should have a parent");

    if let Some(map) = map.as_mut() {
      let paths =
        map.get_sources().map(|source| source.as_path().relative(file_dir)).collect::<Vec<_>>();
      // Here not normalize the windows path, the rollup `sourcemap_path_transform` ctx.options need to original path.
      let sources = paths.iter().map(|x| x.to_string_lossy()).collect::<Vec<_>>();
      map.set_sources(sources.iter().map(std::convert::AsRef::as_ref).collect::<Vec<_>>());
    }

    Ok(vec![PreliminaryAsset {
      origin_chunk: ctx.chunk_idx,
      content,
      map,
      meta: AssetMeta::from(EcmaAssetMeta { rendered_chunk }),
      augment_chunk_hash: None,
      file_dir: file_dir.to_path_buf(),
      preliminary_filename: ctx
        .chunk
        .preliminary_filename
        .clone()
        .expect("should have preliminary filename"),
    }])
  }
}
