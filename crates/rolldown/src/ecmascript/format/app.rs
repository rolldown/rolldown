use rolldown_common::{ChunkKind, Module};
use rolldown_sourcemap::{ConcatSource, RawSource};

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

pub fn render_app(
  ctx: &GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  if let Some(banner) = banner {
    concat_source.add_source(Box::new(RawSource::new(banner)));
  }

  if let Some(intro) = intro {
    concat_source.add_source(Box::new(RawSource::new(intro)));
  }

  // chunk content
  module_sources.into_iter().for_each(|(module_idx, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      let is_runtime = ctx.link_output.runtime.id() == module_idx;
      if !is_runtime {
        concat_source.add_source(Box::new(RawSource::new(format!(
          "rolldown_runtime.define('{}',function(require, module, exports){{\n",
          // Here need to care about virtual module `\0`, the oxc codegen will escape it, so here also escape it
          ctx.link_output.module_table.modules[module_idx].stable_id().escape_default()
        ))));
      }
      for source in emitted_sources {
        concat_source.add_source(source);
      }
      if !is_runtime {
        concat_source.add_source(Box::new(RawSource::new("});".to_string())));
      }
    }
  });

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    if let Module::Ecma(entry_module) = &ctx.link_output.module_table.modules[entry_id] {
      concat_source.add_source(Box::new(RawSource::new(format!(
        "rolldown_runtime.require('{}');",
        entry_module.stable_id.escape_default()
      ))));
    }
  }

  if let Some(outro) = outro {
    concat_source.add_source(Box::new(RawSource::new(outro)));
  }

  if let Some(footer) = footer {
    concat_source.add_source(Box::new(RawSource::new(footer)));
  }

  concat_source
}
