use std::sync::Arc;

use crate::{
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use rolldown_common::{
  AddonRenderContext, EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, ModuleIdx,
  OutputFormat, RenderedModule,
};
use rolldown_error::BuildResult;
use rolldown_plugin::HookAddonArgs;
use rolldown_sourcemap::Source;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use super::format::{cjs::render_cjs, esm::render_esm, iife::render_iife, umd::render_umd};

pub type RenderedModuleSources = Vec<RenderedModuleSource>;

pub struct RenderedModuleSource {
  pub module_idx: ModuleIdx,
  pub module_id: ModuleId,
  pub exec_order: u32,
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
}

impl RenderedModuleSource {
  pub fn new(
    module_idx: ModuleIdx,
    module_id: ModuleId,
    exec_order: u32,
    sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  ) -> Self {
    Self { module_idx, module_id, exec_order, sources }
  }
}

pub struct EcmaGenerator;

impl Generator for EcmaGenerator {
  #[allow(clippy::too_many_lines)]
  async fn instantiate_chunk(ctx: &mut GenerateContext<'_>) -> Result<BuildResult<GenerateOutput>> {
    let mut rendered_modules = FxHashMap::default();
    let module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
    let rendered_module_sources: RenderedModuleSources = ctx
      .chunk
      .modules
      .par_iter()
      .copied()
      .zip(module_id_to_codegen_ret)
      .filter_map(|(id, codegen_ret)| {
        ctx.link_output.module_table[id]
          .as_normal()
          .map(|m| (m, codegen_ret.expect("should have codegen_ret")))
      })
      .map(|(m, codegen_ret)| {
        RenderedModuleSource::new(
          m.idx,
          m.id.clone(),
          m.exec_order,
          render_ecma_module(m, ctx.options, codegen_ret),
        )
      })
      .collect::<Vec<_>>();

    rendered_module_sources.iter().for_each(|rendered_module_source| {
      let RenderedModuleSource { module_idx, module_id, exec_order, sources } =
        rendered_module_source;
      let rendered_exports = ctx.link_output.metas[*module_idx]
        .resolved_exports
        .iter()
        .filter_map(|(key, export)| {
          if ctx.link_output.used_symbol_refs.contains(&export.symbol_ref) {
            Some(key.clone())
          } else {
            None
          }
        })
        .collect::<Vec<_>>();
      rendered_modules.insert(
        module_id.clone(),
        RenderedModule::new(sources.clone(), rendered_exports, *exec_order),
      );
    });

    let rendered_chunk = Arc::new(generate_rendered_chunk(ctx, rendered_modules));

    let hashbang = ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table).and_then(
      |normal_module| {
        normal_module
          .ecma_view
          .hashbang_range
          .map(|range| &normal_module.source[range.start as usize..range.end as usize])
      },
    );

    let directives: Vec<_> = ctx
      .chunk
      .user_defined_entry_module(&ctx.link_output.module_table)
      .or_else(|| {
        ctx.options.preserve_modules.then_some({
          let first_idx = *ctx.chunk.modules.first()?;
          ctx.link_output.module_table[first_idx].as_normal()?
        })
      })
      .map(|normal_module| {
        normal_module
          .ecma_view
          .directive_range
          .iter()
          .map(|range| &normal_module.source[range.start as usize..range.end as usize])
          .collect::<_>()
      })
      .unwrap_or_default();

    let banner = {
      let injection = match ctx.options.banner.as_ref() {
        Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
        None => None,
      };
      ctx
        .plugin_driver
        .banner(HookAddonArgs { chunk: Arc::clone(&rendered_chunk) }, injection.unwrap_or_default())
        .await?
    };

    let intro = {
      let injection = match ctx.options.intro.as_ref() {
        Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
        None => None,
      };
      ctx
        .plugin_driver
        .intro(HookAddonArgs { chunk: Arc::clone(&rendered_chunk) }, injection.unwrap_or_default())
        .await?
    };

    let outro = {
      let injection = match ctx.options.outro.as_ref() {
        Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
        None => None,
      };
      ctx
        .plugin_driver
        .outro(HookAddonArgs { chunk: Arc::clone(&rendered_chunk) }, injection.unwrap_or_default())
        .await?
    };

    let footer = {
      let injection = match ctx.options.footer.as_ref() {
        Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
        None => None,
      };
      ctx
        .plugin_driver
        .footer(HookAddonArgs { chunk: Arc::clone(&rendered_chunk) }, injection.unwrap_or_default())
        .await?
    };

    let mut warnings = vec![];

    let addon_render_context = AddonRenderContext {
      hashbang,
      banner: banner.as_deref(),
      intro: intro.as_deref(),
      outro: outro.as_deref(),
      footer: footer.as_deref(),
      directives: &directives,
    };
    let mut source_joiner = match ctx.options.format {
      OutputFormat::Esm => render_esm(ctx, addon_render_context, &rendered_module_sources),
      OutputFormat::Cjs => {
        match render_cjs(ctx, addon_render_context, &rendered_module_sources, &mut warnings) {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
      OutputFormat::Iife => {
        match render_iife(ctx, addon_render_context, &rendered_module_sources, &mut warnings).await
        {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
      OutputFormat::Umd => {
        match render_umd(ctx, addon_render_context, &rendered_module_sources, &mut warnings).await {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
    };

    ctx.warnings.extend(warnings);

    if ctx.options.experimental.is_attach_debug_info_full() && !ctx.chunk.create_reasons.is_empty()
    {
      source_joiner.prepend_source(format!("//! {}", ctx.chunk.create_reasons.join("\n//! ")));
    }

    let (content, map) = source_joiner.join();

    // Here file path is generated by chunk file name template, it maybe including path segments.
    // So here need to read it's parent directory as file_dir.
    let file_path = ctx.options.cwd.as_path().join(&ctx.options.out_dir).join(
      ctx
        .chunk
        .preliminary_filename
        .as_deref()
        .expect("chunk file name should be generated before rendering")
        .as_str(),
    );
    let file_dir = file_path.parent().expect("chunk file name should have a parent");

    Ok(Ok(GenerateOutput {
      chunks: vec![InstantiatedChunk {
        origin_chunk: ctx.chunk_idx,
        content: content.into(),
        map,
        kind: InstantiationKind::from(EcmaAssetMeta {
          rendered_chunk,
          debug_id: 0,
          imports: vec![],
          dynamic_imports: vec![],
        }),
        augment_chunk_hash: None,
        file_dir: file_dir.to_path_buf(),
        preliminary_filename: ctx
          .chunk
          .preliminary_filename
          .clone()
          .expect("should have preliminary filename"),
      }],
      warnings: std::mem::take(&mut ctx.warnings),
    }))
  }
}
