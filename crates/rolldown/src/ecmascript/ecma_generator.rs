use std::sync::Arc;

use crate::{
  stages::link_stage::LinkStageOutput,
  types::{
    generator::{GenerateContext, GenerateOutput, Generator, MultiModuleChunkRender},
    scan_stage_cache::CachedMultiModuleChunk,
  },
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use oxc_str::CompactStr;
use rolldown_common::{
  AddonRenderContext, ChunkIdx, EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId,
  ModuleIdx, OutputFormat, RenderedModule, RollupRenderedChunk, StrictMode,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::HookAddonArgs;
use rolldown_sourcemap::{Source, SourceJoiner};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rolldown_utils::{concat_string, ecmascript::escape_line_terminators};
use rustc_hash::FxHashMap;

use super::format::utils::is_use_strict_directive;
use super::format::{cjs::render_cjs, esm::render_esm, iife::render_iife, umd::render_umd};

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
    let mut module_id_to_codegen_ret = std::mem::take(&mut ctx.module_id_to_codegen_ret);
    let cached_module_sources = ctx.cached_module_sources;
    let build_rendered_module_sources =
      |ctx: &GenerateContext<'_>,
       module_id_to_codegen_ret: Vec<Option<rolldown_common::ModuleRenderOutput>>|
       -> (RenderedModuleSources, Vec<BuildDiagnostic>) {
        let rendered_pairs: Vec<(RenderedModuleSource, Vec<BuildDiagnostic>)> = ctx
          .chunk
          .modules
          .par_iter()
          .copied()
          .zip(module_id_to_codegen_ret)
          .filter_map(|(id, codegen_ret)| {
            ctx.link_output.module_table[id]
              .as_normal()
              .map(|m| (m, codegen_ret))
          })
          .map(|(m, codegen_ret)| {
            if let Some(codegen_ret) = codegen_ret {
              let render = render_ecma_module(m, ctx.options, codegen_ret);
              (
                RenderedModuleSource::new(m.idx, m.id.clone(), m.exec_order, render.sources),
                render.warnings,
              )
            } else {
              let sources = cached_module_sources
                .and_then(|map| map.get(&m.idx))
                .and_then(|s| s.0.clone())
                .or_else(|| {
                  crate::stages::generate_stage::GenerateStage::finalize_and_render_cached_module(
                    ctx, m,
                  )
                });
              (
                RenderedModuleSource::new(m.idx, m.id.clone(), m.exec_order, sources),
                Vec::new(),
              )
            }
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
        (rendered_module_sources, sourcemap_broken_warnings)
      };

    let need_rendered_modules = !ctx.carried_renders.is_empty()
      || !ctx.plugin_driver.order_by_banner_meta.is_empty()
      || !ctx.plugin_driver.order_by_intro_meta.is_empty()
      || !ctx.plugin_driver.order_by_outro_meta.is_empty()
      || !ctx.plugin_driver.order_by_footer_meta.is_empty()
      || !ctx.plugin_driver.order_by_augment_chunk_hash_meta.is_empty()
      || ctx.options.banner.is_some()
      || ctx.options.intro.is_some()
      || ctx.options.outro.is_some()
      || ctx.options.footer.is_some()
      || ctx.options.post_banner.is_some()
      || ctx.options.post_footer.is_some()
      || rolldown_devtools::trace_action_enabled!();

    let mut built_sources: Option<(RenderedModuleSources, Vec<BuildDiagnostic>)> =
      if need_rendered_modules {
        Some(build_rendered_module_sources(
          ctx,
          std::mem::take(&mut module_id_to_codegen_ret),
        ))
      } else {
        None
      };

    let mut rendered_modules: FxHashMap<ModuleId, RenderedModule> =
      if let Some((ref rendered_module_sources, _)) = built_sources {
        rendered_module_sources
          .iter()
          .map(|rendered_module_source| {
            let RenderedModuleSource { module_idx, module_id, exec_order, sources } =
              rendered_module_source;
            let rendered_exports = retained_export_names(ctx.link_output, *module_idx);
            (module_id.clone(), RenderedModule::new(sources.clone(), rendered_exports, *exec_order))
          })
          .collect()
      } else {
        FxHashMap::default()
      };
    // The inline common chunk records this file carries were finalized and printed for it; their
    // modules are part of this file like its own.
    let mut carried_warnings: Vec<BuildDiagnostic> = Vec::new();
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
            let render = render_ecma_module(module, ctx.options, codegen_ret);
            carried_warnings.extend(render.warnings);
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

    let mut directives: Vec<&str> = ctx
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

    // Apply output.strict option
    match ctx.options.strict {
      StrictMode::Always => {
        let has_use_strict = directives.iter().any(|d| is_use_strict_directive(d));
        if !has_use_strict {
          directives.insert(0, "\"use strict\"");
        }
      }
      StrictMode::Never => {
        directives.retain(|d| !is_use_strict_directive(d));
      }
      StrictMode::Auto => {}
    }

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

    let mut warnings = Vec::new();

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

    let multi_module_chunk = ctx.multi_module_chunk.and_then(|lookup| {
      let key = multi_module_chunk_key(
        ctx,
        addon_render_context(),
        lookup.base_digest,
      )?;
      Some((lookup, key))
    });
    if let Some((lookup, key)) = multi_module_chunk {
      ctx.skip_sourcemap_join = lookup.cached_sourcemaps.contains_key(&key);
    }
    if let Some((lookup, key)) = multi_module_chunk
      && let Some(hit) = lookup.cached.get(&key)
      && (!hit.had_map || ctx.skip_sourcemap_join)
    {
      let content = hit.content.to_string();
      // A skipped join leaves an empty map; finalizing serializes the cached chunk map instead.
      let map = hit.had_map.then(rolldown_sourcemap::SourceMap::default);
      let chunk = instantiated_chunk(ctx, rendered_chunk, content, map, post_banner, post_footer);
      return Ok(Ok(GenerateOutput {
        chunks: vec![chunk],
        warnings,
        multi_module_chunk: Some(MultiModuleChunkRender {
          key,
          skip_sourcemap_join: ctx.skip_sourcemap_join,
          new_entry: None,
        }),
      }));
    }

    let (rendered_module_sources, mut sourcemap_broken_warnings) = match built_sources.take() {
      Some(pair) => pair,
      None => build_rendered_module_sources(ctx, module_id_to_codegen_ret),
    };
    sourcemap_broken_warnings.extend(carried_warnings);
    warnings.splice(0..0, sourcemap_broken_warnings);

    let mut source_joiner = match ctx.options.format {
      OutputFormat::Esm => {
        render_esm(ctx, addon_render_context(), &rendered_module_sources, &carried_sources)
      }
      OutputFormat::Cjs => {
        render_cjs(ctx, addon_render_context(), &rendered_module_sources, &mut warnings)
      }
      OutputFormat::Iife => {
        match render_iife(ctx, addon_render_context(), &rendered_module_sources, &mut warnings)
          .await
        {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
      OutputFormat::Umd => {
        match render_umd(ctx, addon_render_context(), &rendered_module_sources, &mut warnings)
          .await
        {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
    };

    prepend_debug_info(ctx, &mut source_joiner);

    let had_sourcemap = source_joiner.enable_sourcemap;
    if ctx.skip_sourcemap_join {
      source_joiner.enable_sourcemap = false;
    }
    let (content, mut map) = source_joiner.join();
    if ctx.skip_sourcemap_join && had_sourcemap && map.is_none() {
      map = Some(rolldown_sourcemap::SourceMap::default());
    }

    let multi_module_chunk = multi_module_chunk.map(|(lookup, key)| {
      let new_entry =
        (!lookup.cached.contains_key(&key) && warnings.is_empty()).then(|| CachedMultiModuleChunk {
          content: Arc::from(content.as_str()),
          had_map: had_sourcemap,
          finalized: None,
        });
      MultiModuleChunkRender {
        key,
        skip_sourcemap_join: ctx.skip_sourcemap_join,
        new_entry,
      }
    });

    Ok(Ok(GenerateOutput {
      chunks: vec![instantiated_chunk(ctx, rendered_chunk, content, map, post_banner, post_footer)],
      warnings,
      multi_module_chunk,
    }))
  }
}

fn prepend_debug_info(ctx: &GenerateContext<'_>, source_joiner: &mut SourceJoiner<'_>) {
  if ctx.options.experimental.is_attach_debug_info_full() && !ctx.chunk.debug_info.is_empty() {
    // Module ids and chunk names can hold line terminators. Escape them, so that each item
    // stays on its own `//!` line.
    let debug_info_str = ctx
      .chunk
      .debug_info
      .iter()
      .map(|info| escape_line_terminators(info.to_string()))
      .collect::<Vec<_>>()
      .join("\n//! ");
    source_joiner.prepend_source(concat_string!("//! ", debug_info_str));
  }
}

fn instantiated_chunk(
  ctx: &GenerateContext<'_>,
  rendered_chunk: Arc<RollupRenderedChunk>,
  content: String,
  map: Option<rolldown_sourcemap::SourceMap>,
  post_banner: Option<String>,
  post_footer: Option<String>,
) -> InstantiatedChunk {
  // Here file path is generated by chunk file name template, it maybe including path segments.
  // So here need to read it's parent directory as file_dir.
  let preliminary_filename =
    ctx.chunk.preliminary_filename.clone().expect("should have preliminary filename");
  let file_path =
    ctx.options.cwd.as_path().join(&ctx.options.out_dir).join(preliminary_filename.as_str());
  let file_dir = file_path.parent().expect("chunk file name should have a parent");

  InstantiatedChunk {
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
      preliminary_filename: preliminary_filename.clone(),
    }),
    preliminary_filename,
    preliminary_sourcemap_filename: ctx.chunk.preliminary_sourcemap_filename.clone(),
    augment_chunk_hash: None,
    post_banner,
    post_footer,
  }
}

/// Stands in for a chunk's modules when rendering the code around them. The NULs keep it from
/// colliding with rendered imports, exports and addons.
const MODULES_SENTINEL: &str = "\0rolldown:multi-module-chunk:modules\0";

/// Keys a chunk's joined code by `base_digest`, which covers its modules' rendered code in order,
/// and the code that the format renders around those modules: hashbang, directives, addons,
/// imports, exports and debug info. The latter is rendered for real around a sentinel module, so
/// the key cannot miss an input of the format renderer. `SourceJoiner` separates every slot with
/// one newline, so the code around the sentinel and the module code determine the joined code.
fn multi_module_chunk_key(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'_>,
  base_digest: [u128; 2],
) -> Option<[u128; 2]> {
  use std::hash::Hash;
  use xxhash_rust::xxh3::Xxh3;

  if !ctx.chunk.module_groups.is_empty() {
    return None;
  }
  let mut normal_modules = ctx
    .chunk
    .modules
    .iter()
    .copied()
    .filter_map(|id| ctx.link_output.module_table[id].as_normal());
  // CJS hoists a leading runtime module above the imports, so it must not stand in for one.
  let first = normal_modules.next()?;
  if first.idx == ctx.link_output.runtime.id() {
    return None;
  }
  let normal_module_count = 1 + normal_modules.count();
  let sentinel: RenderedModuleSources = vec![RenderedModuleSource::new(
    first.idx,
    first.id.clone(),
    first.exec_order,
    Some(Arc::from(vec![Box::new(MODULES_SENTINEL) as Box<dyn Source + Send + Sync>])),
  )];
  let mut warnings = Vec::new();
  let mut source_joiner = match ctx.options.format {
    OutputFormat::Esm => render_esm(ctx, addon_render_context, &sentinel, &[]),
    OutputFormat::Cjs => render_cjs(ctx, addon_render_context, &sentinel, &mut warnings),
    OutputFormat::Iife | OutputFormat::Umd => return None,
  };
  prepend_debug_info(ctx, &mut source_joiner);
  source_joiner.enable_sourcemap = false;
  let (skeleton, _) = source_joiner.join();

  let mut h0 = Xxh3::with_seed(0x452821e638d01377);
  let mut h1 = Xxh3::with_seed(0xbe5466cf34e90c6c);
  let v = (base_digest, ctx.options.format as u8, normal_module_count, skeleton.as_str());
  v.hash(&mut h0);
  v.hash(&mut h1);
  Some([h0.digest128(), h1.digest128()])
}
