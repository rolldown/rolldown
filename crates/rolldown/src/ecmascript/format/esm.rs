use arcstr::ArcStr;
use itertools::Itertools;
use rolldown_common::{ExportsKind, Specifier, WrapKind};
use rolldown_sourcemap::SourceJoiner;
use rolldown_utils::{concat_string, ecmascript::is_validate_identifier_name};

use crate::{
  ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext,
  utils::chunk::render_chunk_exports::render_chunk_exports,
};

pub fn render_esm<'code>(
  ctx: &mut GenerateContext<'_>,
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

  source_joiner.append_source(render_esm_chunk_imports(ctx));

  if let Some(entry_module) = ctx.chunk.entry_module(&ctx.link_output.module_table) {
    if matches!(entry_module.exports_kind, ExportsKind::Esm) {
      entry_module
        .star_export_module_ids()
        .filter_map(|importee| {
          let importee = &ctx.link_output.module_table.modules[importee];
          importee.as_external().map(|m| &m.name)
        })
        .dedup()
        .for_each(|ext_name| {
          source_joiner.append_source(concat_string!("export * from \"", ext_name, "\"\n"));
        });
    }
  }

  // chunk content
  module_sources.iter().for_each(|(_, _, module_render_output)| {
    if let Some(emitted_sources) = module_render_output {
      for source in emitted_sources.as_ref() {
        source_joiner.append_source(source);
      }
    }
  });

  if let Some(entry_id) = ctx.chunk.entry_module_idx() {
    let entry_meta = &ctx.link_output.metas[entry_id];
    match entry_meta.wrap_kind {
      WrapKind::Esm => {
        // init_xxx()
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        source_joiner.append_source(concat_string!(wrapper_ref_name.as_str(), "();"));
      }
      WrapKind::Cjs => {
        // "export default require_xxx();"
        let wrapper_ref = entry_meta.wrapper_ref.as_ref().unwrap();
        let wrapper_ref_name = ctx.finalized_string_pattern_for_symbol_ref(
          *wrapper_ref,
          ctx.chunk_idx,
          &ctx.chunk.canonical_names,
        );
        source_joiner.append_source(concat_string!(
          "export default ",
          wrapper_ref_name.as_str(),
          "();\n"
        ));
      }
      WrapKind::None => {}
    }
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

fn render_esm_chunk_imports(ctx: &GenerateContext<'_>) -> String {
  let mut s = String::new();

  ctx.chunk.imports_from_other_chunks.iter().for_each(|(exporter_id, items)| {
    let importee_chunk = &ctx.chunk_graph.chunk_table[*exporter_id];
    let mut default_alias = vec![];
    let mut specifiers = items
      .iter()
      .filter_map(|item| {
        let canonical_ref = ctx.link_output.symbol_db.canonical_ref_for(item.import_ref);
        let imported = &ctx.chunk.canonical_names[&canonical_ref];
        let Specifier::Literal(alias) = item.export_alias.as_ref().unwrap() else {
          panic!("should not be star import from other chunks")
        };
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
      // TODO: filename relative to importee
      &ctx.chunk.import_path_for(importee_chunk).into(),
    ));
  });
  // render external imports
  ctx.chunk.imports_from_external_modules.iter().for_each(|(importee_id, named_imports)| {
    let importee = &ctx.link_output.module_table.modules[*importee_id]
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
        };
        let alias = &ctx.chunk.canonical_names[canonical_ref];
        match &item.imported {
          Specifier::Star => {
            has_importee_imported = true;
            s.push_str("import * as ");
            s.push_str(alias);
            s.push_str(" from \"");
            s.push_str(&importee.name);
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
              let imported = if is_validate_identifier_name(imported) {
                imported.clone()
              } else {
                format!("'{imported}'").into()
              };
              Some(concat_string!(imported, " as ", alias))
            }
          }
        }
      })
      .sorted_unstable()
      .dedup()
      .collect::<Vec<_>>();
    default_alias.sort_unstable();

    if !specifiers.is_empty()
      || !default_alias.is_empty()
      || (importee.side_effects.has_side_effects() && !has_importee_imported)
    {
      s.push_str(&create_import_declaration(specifiers, &default_alias, &importee.name));
    }
  });

  s
}

fn create_import_declaration(
  mut specifiers: Vec<String>,
  default_alias: &[ArcStr],
  path: &ArcStr,
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
