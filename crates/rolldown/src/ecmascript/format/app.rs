use rolldown_common::{ChunkKind, Module};
use rolldown_sourcemap::SourceJoiner;

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

pub fn render_app<'code>(
  ctx: &GenerateContext<'_>,
  hashbang: Option<&'code str>,
  banner: Option<&'code str>,
  intro: Option<&'code str>,
  outro: Option<&'code str>,
  footer: Option<&'code str>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }
  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  // chunk content
  module_sources.iter().for_each(|(module_idx, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      source_joiner.append_source(format!(
        "rolldown_runtime.define('{}',function(require, module, exports){{\n",
        // Here need to care about virtual module `\0`, the oxc codegen will escape it, so here also escape it
        ctx.link_output.module_table.modules[*module_idx].stable_id().escape_default()
      ));
      for source in emitted_sources.iter() {
        source_joiner.append_source(source);
      }
      source_joiner.append_source("});".to_string());
    }
  });

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    if let Module::Normal(entry_module) = &ctx.link_output.module_table.modules[entry_id] {
      source_joiner.append_source(format!(
        "rolldown_runtime.require('{}');",
        entry_module.stable_id.escape_default()
      ));
    }
  }

  if let Some(outro) = outro {
    source_joiner.append_source(outro);
  }

  if let Some(footer) = footer {
    source_joiner.append_source(footer);
  }

  source_joiner
}
