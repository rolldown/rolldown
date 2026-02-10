use std::sync::Arc;

use crate::{
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use rolldown_common::{
  AddonRenderContext, EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, ModuleIdx,
  OutputFormat, RenderedModule, TsConfig,
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
  async fn instantiate_chunk(ctx: &mut GenerateContext<'_>) -> Result<BuildResult<GenerateOutput>> {
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

    let rendered_modules: FxHashMap<ModuleId, RenderedModule> = rendered_module_sources
      .iter()
      .map(|rendered_module_source| {
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
        (module_id.clone(), RenderedModule::new(sources.clone(), rendered_exports, *exec_order))
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

    // Check if we should inject "use strict" based on tsconfig settings
    if should_inject_use_strict(ctx) {
      // Only add if not already present and not ESM format
      if !matches!(ctx.options.format, OutputFormat::Esm) {
        let has_use_strict = directives.iter().any(|d| {
          let normalized = d.trim_start_matches(['\'', '"']).trim_end_matches(['\'', '"', ';']);
          normalized == "use strict"
        });
        if !has_use_strict {
          // Insert "use strict" at the beginning
          directives.insert(0, "\"use strict\";");
        }
      }
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
        render_cjs(ctx, addon_render_context, &rendered_module_sources, &mut warnings)
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

    if ctx.options.experimental.is_attach_debug_info_full() && !ctx.chunk.debug_info.is_empty() {
      let debug_info_str =
        ctx.chunk.debug_info.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n//! ");
      source_joiner.prepend_source(format!("//! {debug_info_str}"));
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
        augment_chunk_hash: None,

        post_banner,
        post_footer,
      }],
      warnings: std::mem::take(&mut ctx.warnings),
    }))
  }
}

/// Check if "use strict" should be injected based on tsconfig settings
fn should_inject_use_strict(ctx: &GenerateContext<'_>) -> bool {
  use std::path::Path;
  
  // Only inject when tsconfig is enabled
  let tsconfig = &ctx.options.tsconfig;
  
  match tsconfig {
    TsConfig::Auto(false) => {
      // tsconfig is explicitly disabled
      return false;
    }
    TsConfig::Auto(true) | TsConfig::Manual(_) => {
      // tsconfig is enabled, continue to check alwaysStrict
    }
  }

  // Get the entry module to check its file path
  let entry_module = ctx
    .chunk
    .user_defined_entry_module(&ctx.link_output.module_table)
    .or_else(|| {
      ctx.options.preserve_modules.then_some({
        let first_idx = *ctx.chunk.modules.first()?;
        ctx.link_output.module_table[first_idx].as_normal()?
      })
    });

  let entry_module = match entry_module {
    Some(m) => m,
    None => return false,
  };

  // Convert ModuleId to Path for tsconfig resolution
  let entry_path = Path::new(entry_module.id.as_str());
  
  // Try to resolve tsconfig for the entry file
  let resolved_tsconfig = match ctx.resolver.resolve_tsconfig(&entry_path) {
    Ok(tsconfig) => tsconfig,
    Err(_) => return false,
  };

  // Parse the tsconfig.json file to check for alwaysStrict
  // Since oxc_resolver doesn't expose alwaysStrict, we need to read it manually
  let tsconfig_path = &resolved_tsconfig.path;
  
  // Read and parse the tsconfig.json file
  let Ok(content) = std::fs::read_to_string(tsconfig_path) else {
    return false;
  };
  
  let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
    return false;
  };
  
  // Check for alwaysStrict or strict in compilerOptions
  if let Some(compiler_options) = json.get("compilerOptions") {
    let always_strict = compiler_options.get("alwaysStrict")
      .and_then(|v| v.as_bool())
      .unwrap_or(false);
    
    let strict = compiler_options.get("strict")
      .and_then(|v| v.as_bool())
      .unwrap_or(false);
    
    return always_strict || strict;
  }
  
  false
}
