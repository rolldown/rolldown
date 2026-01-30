use itertools::Itertools as _;
use rolldown_common::ExternalModule;
use rolldown_sourcemap::SourceJoiner;

use crate::{
  ecmascript::ecma_generator::{RenderedModuleSource, RenderedModuleSources},
  types::generator::GenerateContext,
  utils::external_import_interop::external_import_needs_interop,
};

pub mod namespace;

/// Categorizes an external import by how it's used in the chunk.
#[derive(Clone, Copy)]
pub enum ExternalImportKind<'a> {
  /// Exports from this external are actually used in the code.
  Used(&'a ExternalModule),
  /// External is only imported for side effects (no exports used).
  SideEffectOnly(&'a ExternalModule),
}

impl<'a> ExternalImportKind<'a> {
  pub fn module(&self) -> &'a ExternalModule {
    match self {
      ExternalImportKind::Used(m) | ExternalImportKind::SideEffectOnly(m) => m,
    }
  }

  pub fn is_used(&self) -> bool {
    matches!(self, ExternalImportKind::Used(_))
  }
}

pub fn render_factory_parameters(
  ctx: &GenerateContext<'_>,
  externals: &[&ExternalModule],
  has_exports: bool,
) -> String {
  let mut parameters: Vec<&str> = if has_exports { vec!["exports"] } else { vec![] };
  externals.iter().for_each(|external| {
    let symbol_name = ctx
      .link_output
      .symbol_db
      .canonical_name_for_or_original(external.namespace_ref, &ctx.chunk.canonical_names);
    parameters.push(symbol_name);
  });
  parameters.join(", ")
}

pub fn render_chunk_external_imports<'a>(
  ctx: &'a GenerateContext<'_>,
) -> (String, Vec<ExternalImportKind<'a>>) {
  let mut import_code = String::new();

  let externals = ctx
    .chunk
    .direct_imports_from_external_modules
    .iter()
    .filter_map(|(importee_id, named_imports)| {
      let importee = ctx.link_output.module_table[*importee_id]
        .as_external()
        .expect("Should be external module here");

      let external_module_symbol_name = ctx
        .link_output
        .symbol_db
        .canonical_name_for_or_original(importee.namespace_ref, &ctx.chunk.canonical_names);

      if ctx.link_output.used_symbol_refs.contains(&importee.namespace_ref) {
        // Check if this import needs __toESM
        let needs_interop = external_import_needs_interop(named_imports);
        if needs_interop {
          let to_esm_fn_name = ctx.link_output.symbol_db.canonical_name_for_or_original(
            ctx.link_output.runtime.resolve_symbol("__toESM"),
            &ctx.chunk.canonical_names,
          );

          import_code.push_str(external_module_symbol_name);
          import_code.push_str(" = ");
          import_code.push_str(to_esm_fn_name);
          import_code.push('(');
          import_code.push_str(external_module_symbol_name);
          import_code.push_str(");\n");
        }
        Some(ExternalImportKind::Used(importee))
      } else if importee.side_effects.has_side_effects() {
        Some(ExternalImportKind::SideEffectOnly(importee))
      } else {
        None
      }
    })
    .collect_vec();

  (import_code, externals)
}

pub fn render_modules_with_peek_runtime_module_at_first<'a>(
  ctx: &GenerateContext<'_>,
  source_joiner: &mut SourceJoiner<'a>,
  module_sources: &'a RenderedModuleSources,
  import_code: String,
) {
  let mut module_sources_peekable = module_sources.iter().peekable();
  match module_sources_peekable.peek() {
    Some(RenderedModuleSource { module_idx, .. })
      if *module_idx == ctx.link_output.runtime.id() =>
    {
      if let RenderedModuleSource { sources: Some(emitted_sources), .. } =
        module_sources_peekable.next().expect("Must have module")
      {
        for source in emitted_sources.iter() {
          source_joiner.append_source(source);
        }
      }
    }
    _ => {}
  }

  source_joiner.append_source(import_code);

  // chunk content
  module_sources_peekable.for_each(
    |RenderedModuleSource { sources: module_render_output, .. }| {
      if let Some(emitted_sources) = module_render_output {
        for source in emitted_sources.as_ref() {
          source_joiner.append_source(source);
        }
      }
    },
  );
}

pub fn render_chunk_directives<'a, T: Iterator<Item = &'a &'a str>>(directives: T) -> String {
  let mut ret = String::new();
  for d in directives {
    ret.push_str(d);
    if (d).ends_with(';') {
      ret.push('\n');
    } else {
      ret.push_str(";\n");
    }
  }
  ret
}
