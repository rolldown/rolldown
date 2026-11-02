use std::sync::Arc;

use crate::{
  stages::link_stage::LinkStageOutput,
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use oxc_str::CompactStr;
use rolldown_common::{
  AddonRenderContext, ChunkIdx, EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId,
  ModuleIdx, OutputFormat, RenderedModule,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::HookAddonArgs;
use rolldown_sourcemap::Source;
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rolldown_utils::{concat_string, ecmascript::escape_line_terminators};
use rustc_hash::FxHashMap;

use super::format::utils::{chunk_directives, is_use_strict_directive, render_chunk_directives};
use super::format::{cjs::render_cjs, esm::render_esm, iife::prepare_iife, umd::prepare_umd};

pub type RenderedModuleSources = Vec<RenderedModuleSource>;

/// The export names of a module that survive tree shaking, reported on its `RenderedModule`.
fn retained_export_names(link_output: &LinkStageOutput, module_idx: ModuleIdx) -> Vec<CompactStr> {
  link_output.metas[module_idx]
    .resolved_exports
    .iter()
    .filter(|(_, export)| link_output.retained_export_symbols.contains(&export.symbol_ref))
    .map(|(name, _)| name.clone())
    .collect()
}

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
  #[expect(clippy::too_many_lines)]
  async fn instantiate_chunk(ctx: &mut GenerateContext<'_>) -> Result<BuildResult<GenerateOutput>> {
    let module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
    let rendered_pairs: Vec<(RenderedModuleSource, Vec<BuildDiagnostic>)> = ctx
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
        let render = render_ecma_module(m, ctx.options, codegen_ret);
        (
          RenderedModuleSource::new(m.idx, m.id.clone(), m.exec_order, render.sources),
          render.warnings,
        )
      })
      .collect::<Vec<_>>();

    let mut sourcemap_broken_warnings: Vec<BuildDiagnostic> = Vec::new();
    let rendered_module_sources: RenderedModuleSources = rendered_pairs
      .into_iter()
      .map(|(source, warnings)| {
        sourcemap_broken_warnings.extend(warnings);
        source
      })
      .collect();

    let mut rendered_modules: FxHashMap<ModuleId, RenderedModule> = rendered_module_sources
      .iter()
      .map(|rendered_module_source| {
        let RenderedModuleSource { module_idx, module_id, exec_order, sources } =
          rendered_module_source;
        let rendered_exports = retained_export_names(ctx.link_output, *module_idx);
        (module_id.clone(), RenderedModule::new(sources.clone(), rendered_exports, *exec_order))
      })
      .collect();
    // The inline common chunk records this file carries were finalized and printed for it; their
    // modules are part of this file like its own.
    let carried_renders = std::mem::take(&mut ctx.carried_renders);
    let carried_sources: Vec<(ChunkIdx, RenderedModuleSources)> = carried_renders
      .into_iter()
      .map(|carried| {
        let sources = ctx.chunk_graph.chunk_table[carried.record]
          .modules
          .iter()
          .zip(carried.modules)
          .map(|(module_idx, codegen_ret)| {
            let module = ctx.link_output.module_table[*module_idx]
              .as_normal()
              .expect("a record holds only normal modules");
            let render = render_ecma_module(module, ctx.options, codegen_ret.rendered);
            sourcemap_broken_warnings.extend(render.warnings);
            let rendered_exports = retained_export_names(ctx.link_output, *module_idx);
            rendered_modules.insert(
              module.id.clone(),
              RenderedModule::new(render.sources.clone(), rendered_exports, module.exec_order),
            );
            RenderedModuleSource::new(
              module.idx,
              module.id.clone(),
              module.exec_order,
              render.sources,
            )
          })
          .collect();
        (carried.record, sources)
      })
      .collect();

    let rendered_chunk = Arc::new(generate_rendered_chunk(ctx, rendered_modules));

    let hashbang = ctx.chunk.user_defined_entry_module(&ctx.link_output.module_table).and_then(
      |normal_module| {
        normal_module
          .ecma_view
          .hashbang_range
          .map(|range| &normal_module.source[range.start as usize..range.end as usize])
      },
    );

    let directives = chunk_directives(ctx);

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

    let post_banner = match ctx.options.post_banner.as_ref() {
      Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
      None => None,
    };

    let post_footer = match ctx.options.post_footer.as_ref() {
      Some(hook) => hook.call(Arc::clone(&rendered_chunk)).await?,
      None => None,
    };

    let mut warnings = sourcemap_broken_warnings;

    // Warn when multiple shebang sources would produce duplicate shebangs in the output.
    // UMD format silently drops the entry hashbang, so it doesn't count as a shebang source.
    let entry_has_shebang = hashbang.is_some() && !matches!(ctx.options.format, OutputFormat::Umd);
    let banner_has_shebang = banner.as_ref().is_some_and(|b| b.starts_with("#!"));
    let post_banner_has_shebang = post_banner.as_ref().is_some_and(|pb| pb.starts_with("#!"));

    if (entry_has_shebang && (banner_has_shebang || post_banner_has_shebang))
      || (banner_has_shebang && post_banner_has_shebang)
    {
      let filename = ctx
        .chunk
        .preliminary_filename
        .as_deref()
        .expect("chunk file name should be generated before rendering")
        .to_string();
      if entry_has_shebang && banner_has_shebang {
        warnings.push(
          BuildDiagnostic::duplicate_shebang(filename.clone(), "banner").with_severity_warning(),
        );
      }
      if entry_has_shebang && post_banner_has_shebang {
        warnings.push(
          BuildDiagnostic::duplicate_shebang(filename.clone(), "postBanner")
            .with_severity_warning(),
        );
      }
      if banner_has_shebang && post_banner_has_shebang {
        warnings.push(
          BuildDiagnostic::duplicate_shebang(filename, "banner and postBanner")
            .with_severity_warning(),
        );
      }
    }

    let addon_render_context = || AddonRenderContext {
      hashbang,
      banner: banner.as_deref(),
      intro: intro.as_deref(),
      outro: outro.as_deref(),
      footer: footer.as_deref(),
      directives: &directives,
    };
    let debug_header = (ctx.options.experimental.is_attach_debug_info_full()
      && !ctx.chunk.debug_info.is_empty())
    .then(|| {
      // Module ids and chunk names can hold line terminators. Escape them, so that each item
      // stays on its own `//!` line.
      let debug_info_str = ctx
        .chunk
        .debug_info
        .iter()
        .map(|info| escape_line_terminators(info.to_string()))
        .collect::<Vec<_>>()
        .join("\n//! ");
      concat_string!("//! ", debug_info_str)
    });
    let mut wrapped_plan = match ctx.options.format {
      OutputFormat::Iife => match prepare_iife(ctx, addon_render_context(), &mut warnings).await {
        Ok(plan) => Some(plan),
        Err(errors) => return Ok(Err(errors)),
      },
      OutputFormat::Umd => match prepare_umd(ctx, addon_render_context(), &mut warnings).await {
        Ok(plan) => Some(plan),
        Err(errors) => return Ok(Err(errors)),
      },
      _ if ctx.defer_ast
        && (ctx.options.is_dev_mode_enabled()
          || ctx.inline_state.has_records()
          || crate::stages::generate_stage::chunk_ast_dce::complete_program(
            ctx.options,
            ctx.plugin_driver,
          )) =>
      {
        Some(crate::ecmascript::WrappedPlan::flat(ctx, &addon_render_context()))
      }
      _ => None,
    };
    let mut source_joiner = if let Some(plan) = &wrapped_plan {
      plan.render(ctx, &rendered_module_sources, &carried_sources)
    } else {
      match ctx.options.format {
        OutputFormat::Esm => {
          render_esm(ctx, addon_render_context(), &rendered_module_sources, &carried_sources)
        }
        OutputFormat::Cjs => {
          render_cjs(ctx, addon_render_context(), &rendered_module_sources, &mut warnings)
        }
        OutputFormat::Iife | OutputFormat::Umd => {
          unreachable!("wrapped formats have a rendering plan")
        }
      }
    };

    if let Some(header) = &debug_header {
      source_joiner.prepend_source(header.as_str());
    }

    let t = rolldown_utils::poc_prof::Timer::start();
    let (content, mut map) = source_joiner.join();
    drop(source_joiner);
    // PoC `global` mode: module maps may name other modules of the chunk as sources.
    if ctx.chunk_ast_dce.as_ref().is_some_and(|out| out.space.is_some()) {
      map = map.map(crate::stages::generate_stage::chunk_ast_dce::dedupe_sources);
    }
    t.stop(rolldown_utils::poc_prof::Phase::Join);

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

    let envelope = if ctx.defer_ast && wrapped_plan.is_none() {
      let mut prefix = String::new();
      let remove_whitespace = matches!(
        &ctx.options.minify,
        rolldown_common::MinifyOptions::Enabled(options) if options.remove_whitespace
      );
      if (ctx.options.comments.legal || !remove_whitespace)
        && let Some(header) = &debug_header
      {
        prefix.push_str(header);
        prefix.push('\n');
      }
      if let Some(hashbang) = hashbang {
        prefix.push_str(hashbang);
        prefix.push('\n');
      }
      if let Some(banner) = &banner {
        prefix.push_str(banner);
        prefix.push('\n');
      }
      prefix.push_str(&render_chunk_directives(directives.iter().filter(|directive| {
        !matches!(ctx.options.format, OutputFormat::Esm) || !is_use_strict_directive(directive)
      })));
      let mut suffix = String::new();
      if let Some(footer) = &footer {
        suffix.push_str(footer);
      }
      Some((prefix, suffix))
    } else {
      None
    };
    let property_cache =
      ctx.chunk_ast_dce.as_mut().and_then(|output| output.property_mangle_cache.take());
    Ok(Ok(GenerateOutput {
      pending_ast: None,
      minify_rendered_text: false,
      wrapped_plan: if ctx.defer_ast {
        let mut plan = wrapped_plan.take();
        if let Some(plan) = &mut plan
          && let Some(header) = debug_header
        {
          if let Some(ast) = &mut plan.ast {
            ast.outer_prefix.insert(0, '\n');
            ast.outer_prefix.insert_str(0, &header);
          }
          plan.prefix.insert(0, header);
        }
        plan
      } else {
        None
      },
      envelope,
      property_cache,
      chunks: vec![InstantiatedChunk {
        originate_from: ctx.chunk_idx,
        content: content.into(),
        map,
        kind: InstantiationKind::from(EcmaAssetMeta {
          rendered_chunk,
          debug_id: 0,
          imports: vec![],
          dynamic_imports: vec![],
          file_dir: file_dir.to_path_buf(),
          sourcemap_filename: None,
          preliminary_filename: ctx
            .chunk
            .preliminary_filename
            .clone()
            .expect("should have preliminary filename"),
        }),
        preliminary_filename: ctx
          .chunk
          .preliminary_filename
          .clone()
          .expect("should have preliminary filename"),
        preliminary_sourcemap_filename: ctx.chunk.preliminary_sourcemap_filename.clone(),
        augment_chunk_hash: None,
        post_banner,
        post_footer,
      }],
      warnings,
    }))
  }
}
