mod ast_visit;
mod dynamic_import_to_glob;
mod utils;

use std::{borrow::Cow, pin::Pin, sync::Arc};

use oxc::ast_visit::VisitJs;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin,
  PluginContext, SharedLoadPluginContext,
};
use rolldown_plugin_utils::parse_program;
use rolldown_std_utils::relative_path_as_js_specifier;
use rolldown_utils::{
  futures::{block_on, block_on_spawn_all},
  pattern_filter::StringOrRegex,
};
use sugar_path::SugarPath as _;

pub const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

pub type ResolverFn = dyn Fn(String, String) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<String>>> + Send>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Default)]
pub struct ViteDynamicImportVarsPlugin {
  pub sourcemap: bool,
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  #[debug(skip)]
  pub resolver: Option<Arc<ResolverFn>>,
}

impl Plugin for ViteDynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:vite-dynamic-import-vars")
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    Ok((args.specifier == DYNAMIC_IMPORT_HELPER).then_some(HookResolveIdOutput {
      id: arcstr::literal!(DYNAMIC_IMPORT_HELPER),
      ..Default::default()
    }))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == DYNAMIC_IMPORT_HELPER).then_some(HookLoadOutput {
      code: arcstr::literal!(include_str!("dynamic-import-helper.js")),
      ..Default::default()
    }))
  }

  async fn transform(
    &self,
    ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !self.filter(args.id, ctx.cwd()) {
      return Ok(None);
    }
    if utils::has_dynamic_import(args.code) {
      let rewrite = ast_visit::RewriteContext {
        ctx: &ctx,
        source_text: args.code,
        root: ctx.cwd(),
        importer: args.id.as_path(),
      };

      // This scope parses the code. It then visits the AST. Only owned data
      // leaves the scope. The code below therefore holds no reference to the
      // AST arena.
      let (mut edits, async_imports) = {
        let allocator = oxc::allocator::Allocator::default();
        let Some(parser_ret) = parse_program(&allocator, args.code, args.module_type, args.id)?
        else {
          return Ok(None);
        };
        let mut visitor = ast_visit::DynamicImportVarsVisit {
          rewrite,
          comments: &parser_ret.program.comments,
          current_comment: 0,
          async_imports: Vec::default(),
          edits: Vec::default(),
        };

        visitor.visit_program(&parser_ret.program);

        (visitor.edits, visitor.async_imports)
      };

      if !async_imports.is_empty()
        && let Some(resolver) = &self.resolver
      {
        let task = async_imports
          .iter()
          .map(|pending| async { resolver(pending.glob.clone(), args.id.to_string()).await.ok()? });

        let importer = args.id.as_path().parent().unwrap();
        let result = block_on(block_on_spawn_all(task));
        for (pending, item) in async_imports.iter().zip(result) {
          if let Some(id) = item {
            let id = relative_path_as_js_specifier(id, importer);
            if id == "." {
              continue;
            }

            if let Some(edit) = rewrite.build_edit(
              &id,
              pending.import_span,
              pending.source_span,
              &pending.first_quasi_raw,
            ) {
              edits.push(edit);
            }
          }
        }
      }

      if !edits.is_empty() {
        let mut magic_string = string_wizard::MagicString::new(args.code);
        for edit in edits {
          magic_string
            .update(edit.start, edit.end, edit.replacement)
            .expect("update should not fail in dynamic import vars plugin");
        }
        magic_string.prepend(format!(
          "import __variableDynamicImportRuntimeHelper from \"{DYNAMIC_IMPORT_HELPER}\";"
        ));
        return Ok(Some(HookTransformOutput {
          code: Some(magic_string.to_string()),
          map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
            magic_string.source_map(string_wizard::SourceMapOptions {
              hires: string_wizard::Hires::Boundary,
              source: args.id.into(),
              ..Default::default()
            })
          }),
          ..Default::default()
        }));
      }
    }
    Ok(None)
  }
}
