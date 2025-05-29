use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{AddonRenderContext, ExportsKind, Specifier};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{concat_string, ecmascript::to_module_import_export_name};

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::{render_chunk_exports, render_wrapped_entry_chunk},
};

use super::utils::render_chunk_directives;

#[allow(clippy::needless_pass_by_value)]
pub fn render_esm<'code>(
  ctx: &GenerateContext<'_>,
  addon_render_context: AddonRenderContext<'code>,
  module_sources: &'code RenderedModuleSources,
) -> SourceJoiner<'code> {
  let mut source_joiner = SourceJoiner::default();
  let AddonRenderContext { hashbang, banner, intro, outro, footer, directives } =
    addon_render_context;

  if let Some(hashbang) = hashbang {
    source_joiner.append_source(hashbang);
  }

  if let Some(banner) = banner {
    source_joiner.append_source(banner);
  }

  // https://github.com/evanw/esbuild/blob/d34e79e2a998c21bb71d57b92b0017ca11756912/internal/linker/linker.go#L5686-L5698
  if !directives.is_empty() {
    source_joiner.append_source(render_chunk_directives(
      directives.iter().filter(|d| &d[1..d.len() - 1] != "use strict"),
    ));
    source_joiner.append_source("");
  }

  if let Some(intro) = intro {
    source_joiner.append_source(intro);
  }

  if let Some(imports) = render_esm_chunk_imports(ctx) {
    source_joiner.append_source(imports);
  }

  if let Some(entry_module) = ctx.chunk.entry_module(&ctx.link_output.module_table) {
    if matches!(entry_module.exports_kind, ExportsKind::Esm) {
      entry_module
        .star_export_module_ids()
        .filter_map(|importee| {
          let importee = &ctx.link_output.module_table[importee];
          importee.as_external().map(|m| m.get_import_path(ctx.chunk))
        })
        .dedup()
        .for_each(|ext_name| {
          source_joiner.append_source(concat_string!("export * from \"", ext_name, "\"\n"));
        });
    }
  }

  // chunk content
  module_sources.iter().for_each(
    |RenderedModuleSource { sources: module_render_output, .. }| {
      if let Some(emitted_sources) = module_render_output {
        for source in emitted_sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    },
  );

  if let Some(source) = render_wrapped_entry_chunk(ctx, None) {
    source_joiner.append_source(source);
  }

  if let Some(exports) = render_chunk_exports(ctx, None) {
    if !exports.is_empty() {
      source_joiner.append_source(exports);
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

fn render_esm_chunk_imports(ctx: &GenerateContext<'_>) -> Option<String> {
  let mut s = String::new();
  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let mut default_alias = vec![];
    let mut specifiers = items
      .iter()
      .filter_map(|item| {
        let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(item.import_ref);
        let imported = &ctx.chunk.canonical_names[&canonical_ref];
        let alias = &ctx.render_export_items_index_vec[*exporter_id]
          .get(&item.import_ref)
          .expect("should have export item index")[0];
        if alias == imported {
          Some(alias.as_str().into())
        } else {
          if alias.as_str() == "default" {
            default_alias.push(imported.as_str().into());
            return None;
          }
          Some(concat_string!(alias, " as ", imported))
        }
      })
      .collect::<Vec<_>>();
    specifiers.sort_unstable();

    s.push_str(&create_import_declaration(
      specifiers,
      &default_alias,
      &ctx.chunk.import_path_for(importee_chunk),
    ));
  });

  // render external imports
  ctx.chunk.imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &ctx.link_output.module_table[*importee_id]
      .as_external()
      .expect("Should be external module here");
    let mut has_importee_imported = false;
    let mut default_alias = vec![];
    let specifiers = named_imports
      .iter()
      .filter_map(|item| {
        let canonical_ref = &ctx.link_output.symbol_db.canonical_ref_for(item.imported_as);
        if !ctx.link_output.used_symbol_refs.contains(canonical_ref) {
          return None;
        }
        let alias = &ctx.chunk.canonical_names[canonical_ref];
        match &item.imported {
          Specifier::Star => {
            has_importee_imported = true;
            s.push_str("import * as ");
            s.push_str(alias);
            s.push_str(" from \"");
            s.push_str(&importee.get_import_path(ctx.chunk));
            s.push_str("\";\n");
            None
          }
          Specifier::Literal(imported) => {
            if alias == imported {
              Some(alias.as_str().into())
            } else {
              if imported.as_str() == "default" {
                default_alias.push(alias.as_str().into());
                return None;
              }
              let imported = to_module_import_export_name(imported);
              Some(concat_string!(imported, " as ", alias))
            }
          }
        }
      })
      .sorted_unstable()
      .dedup()
      .collect::<Vec<_>>();
    default_alias.sort_unstable();
    default_alias.dedup();

    if !specifiers.is_empty()
      || !default_alias.is_empty()
      || (importee.side_effects.has_side_effects() && !has_importee_imported)
    {
      s.push_str(&create_import_declaration(
        specifiers,
        &default_alias,
        &importee.get_import_path(ctx.chunk),
      ));
    }
  });
  (!s.is_empty()).then_some(s)
}

fn create_import_declaration(
  mut specifiers: Vec<String>,
  default_alias: &[ArcStr],
  path: &str,
) -> String {
  let mut ret = String::new();
  let first_default_alias = match &default_alias {
    [] => None,
    [first] => Some(first),
    [first, rest @ ..] => {
      specifiers.extend(rest.iter().map(|item| concat_string!("default as ", item)));
      Some(first)
    }
  };
  if !specifiers.is_empty() {
    ret.push_str("import ");
    if let Some(first_default_alias) = first_default_alias {
      ret.push_str(first_default_alias);
      ret.push_str(", ");
    }
    ret.push_str("{ ");
    ret.push_str(&specifiers.join(", "));
    ret.push_str(" } from \"");
    ret.push_str(path);
    ret.push_str("\";\n");
  } else if let Some(first_default_alias) = first_default_alias {
    ret.push_str("import ");
    ret.push_str(first_default_alias);
    ret.push_str(" from \"");
    ret.push_str(path);
    ret.push_str("\";\n");
  } else {
    ret.push_str("import \"");
    ret.push_str(path);
    ret.push_str("\";\n");
  }
  ret
}
