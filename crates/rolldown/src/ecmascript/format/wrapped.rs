use rolldown_sourcemap::SourceJoiner;

use crate::{ecmascript::ecma_generator::RenderedModuleSources, types::generator::GenerateContext};

use super::utils::render_modules_with_peek_runtime_module_at_first;

/// Generated wrapper fragments around the runtime, external imports and chunk modules.
/// Keep each fragment separate: `SourceJoiner` inserts a newline between source slots.
pub struct WrappedPlan {
  pub prefix: Vec<String>,
  pub imports: String,
  pub suffix: Vec<String>,
  pub runtime_before_imports: bool,
  pub ast: Option<super::wrapper_ast::WrapperAstPlan>,
}

impl WrappedPlan {
  pub fn flat(ctx: &GenerateContext<'_>, addons: &rolldown_common::AddonRenderContext<'_>) -> Self {
    use super::{cjs, esm, utils};
    use crate::utils::chunk::render_chunk_exports::{
      render_chunk_exports, render_wrapped_entry_chunk,
    };

    let is_cjs = matches!(ctx.options.format, rolldown_common::OutputFormat::Cjs);
    let mut prefix = Vec::new();
    let mut suffix = Vec::new();
    prefix.extend(addons.hashbang.map(str::to_owned));
    prefix.extend(addons.banner.map(str::to_owned));
    let directives = utils::render_chunk_directives(
      addons
        .directives
        .iter()
        .filter(|directive| is_cjs || !utils::is_use_strict_directive(directive)),
    );
    if !directives.is_empty() {
      prefix.push(directives);
    }
    prefix.extend(addons.intro.map(str::to_owned));
    let imports = if is_cjs {
      prefix.extend(cjs::render_cjs_namespace_markers(ctx).map(str::to_owned));
      cjs::render_cjs_chunk_imports(ctx)
    } else {
      rolldown_utils::concat_string!(
        esm::render_esm_chunk_imports(ctx).unwrap_or_default(),
        esm::render_esm_external_star_exports(ctx).unwrap_or_default()
      )
    };
    let export_mode = is_cjs.then_some(&ctx.chunk.output_exports);
    suffix.extend(render_wrapped_entry_chunk(ctx, export_mode));
    suffix.extend(render_chunk_exports(ctx, export_mode));
    suffix.extend(addons.outro.map(str::to_owned));
    suffix.extend(addons.footer.map(str::to_owned));
    let ast = (ctx.defer_ast
      && !crate::stages::generate_stage::chunk_ast_dce::complete_program(
        ctx.options,
        ctx.plugin_driver,
      ))
    .then(|| {
      let marker = is_cjs
        && matches!(ctx.chunk.output_exports, rolldown_common::OutputExports::Named)
        && ctx
          .chunk
          .user_defined_entry_module(&ctx.link_output.module_table)
          .is_some_and(|module| module.exports_kind.is_esm());
      let has_default_export =
        crate::utils::chunk::render_chunk_exports::get_chunk_export_names_with_ctx(ctx)
          .iter()
          .any(|name| name.as_str() == "default");
      super::wrapper_ast::WrapperAstPlan {
        kind: super::wrapper_ast::WrapperKind::Flat,
        name: None,
        extend: false,
        has_exports: false,
        named_exports: false,
        parameters: vec![],
        globals: vec![],
        es_module: marker
          && crate::utils::chunk::namespace_marker::determine_es_module(
            ctx.options.es_module,
            has_default_export,
          ),
        to_string_tag: marker && ctx.options.generated_code.symbols && ctx.chunk.is_entry_point(),
        tail: suffix[..suffix.len() - usize::from(addons.footer.is_some())].join("\n"),
        outer_prefix: addons.banner.unwrap_or_default().to_string(),
        outer_suffix: addons.footer.unwrap_or_default().to_string(),
        hashbang: addons.hashbang.map(|value| value.trim_start_matches("#!").to_string()),
        directives: addons
          .directives
          .iter()
          .filter(|directive| is_cjs || !utils::is_use_strict_directive(directive))
          .map(|value| (*value).to_string())
          .collect(),
        factory_return: None,
      }
    });
    Self { prefix, imports, suffix, runtime_before_imports: is_cjs, ast }
  }

  pub fn render<'a>(
    &'a self,
    ctx: &GenerateContext<'a>,
    module_sources: &'a RenderedModuleSources,
    carried_sources: &'a [(rolldown_common::ChunkIdx, RenderedModuleSources)],
  ) -> SourceJoiner<'a> {
    let mut source_joiner = SourceJoiner::default();
    for source in &self.prefix {
      source_joiner.append_source(source.as_str());
    }
    if self.runtime_before_imports {
      render_modules_with_peek_runtime_module_at_first(
        ctx,
        &mut source_joiner,
        module_sources,
        self.imports.as_str(),
      );
    } else {
      if !self.imports.is_empty() {
        source_joiner.append_source(self.imports.as_str());
      }
      super::share_factory::render_inline_records(ctx, &mut source_joiner, carried_sources);
      super::esm::render_chunk_content(ctx, module_sources, &mut source_joiner);
    }
    for source in &self.suffix {
      source_joiner.append_source(source.as_str());
    }
    source_joiner
  }
}
