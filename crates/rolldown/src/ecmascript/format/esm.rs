use crate::ecmascript::format::AppendRawString;
use crate::{
  ecmascript::ecma_generator::RenderedModuleSources,
  types::generator::GenerateContext,
  utils::chunk::{
    collect_render_chunk_imports::{
      collect_render_chunk_imports, RenderImportDeclarationSpecifier,
    },
    render_chunk_exports::render_chunk_exports,
  },
};
use itertools::Itertools;
use rolldown_common::{ChunkKind, ExportsKind, Module, WrapKind};
use rolldown_sourcemap::ConcatSource;

pub fn render_esm(
  ctx: &mut GenerateContext<'_>,
  module_sources: RenderedModuleSources,
  banner: Option<String>,
  footer: Option<String>,
  intro: Option<String>,
  outro: Option<String>,
) -> ConcatSource {
  let mut concat_source = ConcatSource::default();

  concat_source.append_optional_raw_string(banner);

  concat_source.append_optional_raw_string(intro);

  concat_source.append_raw_string(render_esm_chunk_imports(ctx));

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    if let Module::Ecma(entry_module) = &ctx.link_output.module_table.modules[entry_id] {
      if matches!(entry_module.exports_kind, ExportsKind::Esm) {
        entry_module
          .star_export_module_ids()
          .filter_map(|importee| {
            let importee = &ctx.link_output.module_table.modules[importee];
            match importee {
              Module::External(ext) => Some(&ext.name),
              Module::Ecma(_) => None,
            }
          })
          .dedup()
          .for_each(|ext_name| {
            let import_stmt = format!("export * from \"{}\"\n", &ext_name);
            concat_source.append_raw_string(import_stmt);
          });
      }
    }
  }

  // chunk content
  module_sources.into_iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources {
        concat_source.add_source(source);
      }
    }
  });

  if let ChunkKind::EntryPoint { module: entry_id, .. } = ctx.chunk.kind {
    let entry_meta = &ctx.link_output.metas[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        // init_xxx()
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name =
          ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
        concat_source.append_raw_string(format!("{wrapper_ref_name}();"));
      }
      WrapKind::Cjs => {
        // "export default require_xxx();"
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name =
          ctx.link_output.symbols.canonical_name_for(*wrapper_ref, &ctx.chunk.canonical_names);
        concat_source.append_raw_string(format!("export default {wrapper_ref_name}();\n"));
      }
      WrapKind::None => {}
    }
  }

  if let Some(exports) = render_chunk_exports(ctx, None) {
    if !exports.is_empty() {
      concat_source.append_raw_string(exports);
    }
  }

  concat_source.append_optional_raw_string(outro);

  concat_source.append_optional_raw_string(footer);

  concat_source
}

fn render_esm_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let render_import_stmts =
    collect_render_chunk_imports(ctx.chunk, ctx.link_output, ctx.chunk_graph);

  let mut s = String::new();
  render_import_stmts.iter().for_each(|stmt| {
    let path = &stmt.path;
    match &stmt.specifiers {
      RenderImportDeclarationSpecifier::ImportSpecifier(specifiers) => {
        if specifiers.is_empty() {
          s.push_str(&format!("import \"{path}\";\n",));
        } else {
          let specifiers = specifiers
            .iter()
            .map(|specifier| {
              if let Some(alias) = &specifier.alias {
                format!("{} as {alias}", specifier.imported)
              } else {
                specifier.imported.to_string()
              }
            })
            .collect::<Vec<_>>();
          s.push_str(&format!("import {{ {} }} from \"{path}\";\n", specifiers.join(", ")));
        }
      }
      RenderImportDeclarationSpecifier::ImportStarSpecifier(alias) => {
        s.push_str(&format!("import * as {alias} from \"{path}\";\n",));
      }
    }
  });

  s
}
