use std::sync::Arc;

use crate::{
  types::generator::{GenerateContext, GenerateOutput, Generator},
  utils::{chunk::generate_rendered_chunk, render_ecma_module::render_ecma_module},
};

use anyhow::Result;
use rolldown_common::{
  AddonRenderContext, EcmaAssetMeta, InstantiatedChunk, InstantiationKind, ModuleId, ModuleIdx,
  OutputFormat, RenderedModule, StrictMode,
};
use rolldown_error::{BuildDiagnostic, BuildResult};
use rolldown_plugin::HookAddonArgs;
use rolldown_sourcemap::{Source, SourceJoiner, SourceMap};
#[cfg(not(target_family = "wasm"))]
use rolldown_utils::rayon::IndexedParallelIterator;
use rolldown_utils::rayon::{IntoParallelRefIterator, ParallelIterator};
use rustc_hash::FxHashMap;

use super::format::utils::is_use_strict_directive;
use super::format::{cjs::render_cjs, esm::render_esm, iife::render_iife, umd::render_umd};

pub type RenderedModuleSources = Vec<RenderedModuleSource>;

pub struct RenderedModuleSource {
  pub module_idx: ModuleIdx,
  pub module_id: ModuleId,
  pub exec_order: u32,
  pub sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  sourcemap: Option<(usize, SourceMap)>,
}

impl RenderedModuleSource {
  pub fn new(
    module_idx: ModuleIdx,
    module_id: ModuleId,
    exec_order: u32,
    mut sources: Option<Arc<[Box<dyn Source + Send + Sync>]>>,
  ) -> Self {
    // Detach maps before `sources` is cloned into `RenderedModule` for plugin hooks.
    // The source text stays shared through the Arc; chunk rendering owns the maps.
    let mut detached_sourcemap = None;
    if let Some(sources) = sources.as_mut() {
      if let Some(sources) = Arc::get_mut(sources) {
        for (index, source) in sources.iter_mut().enumerate() {
          let sourcemap =
            source.as_mut().take_sourcemap().or_else(|| source.as_ref().sourcemap().cloned());
          if let Some(sourcemap) = sourcemap {
            assert!(
              detached_sourcemap.replace((index, sourcemap)).is_none(),
              "a rendered module should contain at most one sourcemap"
            );
          }
        }
      } else {
        // Custom renderers may already share the source array. Preserve behavior
        // by cloning those maps; the normal codegen path always takes the branch above.
        for (index, source) in sources.iter().enumerate() {
          if let Some(sourcemap) = source.sourcemap().cloned() {
            assert!(
              detached_sourcemap.replace((index, sourcemap)).is_none(),
              "a rendered module should contain at most one sourcemap"
            );
          }
        }
      }
    }
    Self { module_idx, module_id, exec_order, sources, sourcemap: detached_sourcemap }
  }

  pub fn append_sources(&mut self, source_joiner: &mut SourceJoiner<'_>) {
    let Some(sources) = &self.sources else {
      return;
    };

    for index in 0..sources.len() {
      let sourcemap = self
        .sourcemap
        .take_if(|(source_index, _)| *source_index == index)
        .map(|(_, sourcemap)| sourcemap);
      source_joiner.append_source(RenderedChunkSource {
        sources: Arc::clone(sources),
        index,
        sourcemap,
      });
    }
    debug_assert!(self.sourcemap.is_none());
  }
}

struct RenderedChunkSource {
  sources: Arc<[Box<dyn Source + Send + Sync>]>,
  index: usize,
  sourcemap: Option<SourceMap>,
}

impl Source for RenderedChunkSource {
  fn sourcemap(&self) -> Option<&SourceMap> {
    self.sourcemap.as_ref()
  }

  fn take_sourcemap(&mut self) -> Option<SourceMap> {
    self.sourcemap.take()
  }

  fn content(&self) -> &str {
    self.sources[self.index].content()
  }

  fn lines_count(&self) -> u32 {
    self.sources[self.index].lines_count()
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
    let mut rendered_module_sources: RenderedModuleSources = rendered_pairs
      .into_iter()
      .map(|(source, warnings)| {
        sourcemap_broken_warnings.extend(warnings);
        source
      })
      .collect();

    // Maps were detached in `RenderedModuleSource::new`, so this clone shares
    // only source text with the plugin-facing module view.
    let rendered_modules: FxHashMap<ModuleId, RenderedModule> = rendered_module_sources
      .iter()
      .map(|rendered_module_source| {
        let RenderedModuleSource { module_idx, module_id, exec_order, sources, .. } =
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

    let addon_render_context = AddonRenderContext {
      hashbang,
      banner: banner.as_deref(),
      intro: intro.as_deref(),
      outro: outro.as_deref(),
      footer: footer.as_deref(),
      directives: &directives,
    };
    let mut source_joiner = match ctx.options.format {
      OutputFormat::Esm => render_esm(ctx, addon_render_context, &mut rendered_module_sources),
      OutputFormat::Cjs => {
        render_cjs(ctx, addon_render_context, &mut rendered_module_sources, &mut warnings)
      }
      OutputFormat::Iife => {
        match render_iife(ctx, addon_render_context, &mut rendered_module_sources, &mut warnings)
          .await
        {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
      OutputFormat::Umd => {
        match render_umd(ctx, addon_render_context, &mut rendered_module_sources, &mut warnings)
          .await
        {
          Ok(source_joiner) => source_joiner,
          Err(errors) => return Ok(Err(errors)),
        }
      }
    };

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
      warnings,
    }))
  }
}

#[cfg(test)]
mod tests {
  use std::{borrow::Cow, sync::Arc};

  use rolldown_common::{ModuleIdx, RenderedModule};
  use rolldown_sourcemap::{Source, SourceJoiner, SourceMap, SourceMapSource};

  use super::RenderedModuleSource;

  #[test]
  fn rendered_module_keeps_code_while_chunk_takes_its_sourcemap() {
    let map = SourceMap::new(
      None,
      vec![],
      None,
      vec![Cow::Borrowed("entry.js")],
      vec![Some(Cow::Borrowed("entry source"))],
      Box::new([]),
      None,
    );
    let source: Box<dyn Source + Send + Sync> =
      Box::new(SourceMapSource::new("entry();".to_string(), map));
    let sources: Arc<[Box<dyn Source + Send + Sync>]> = vec![source].into();

    let mut rendered_source =
      RenderedModuleSource::new(ModuleIdx::new(0), "entry.js".into(), 0, Some(sources));

    let plugin_module =
      RenderedModule::new(rendered_source.sources.clone(), Vec::new(), rendered_source.exec_order);
    assert_eq!(plugin_module.code().as_deref(), Some("entry();"));
    assert!(rendered_source.sources.as_ref().unwrap()[0].sourcemap().is_none());

    let mut joiner = SourceJoiner::default();
    rendered_source.append_sources(&mut joiner);
    let (_, chunk_map) = joiner.join();
    let chunk_map = chunk_map.expect("chunk rendering should own the detached map");
    assert_eq!(chunk_map.get_source(0), Some("entry.js"));
    assert_eq!(chunk_map.get_source_content(0), Some("entry source"));
  }
}
