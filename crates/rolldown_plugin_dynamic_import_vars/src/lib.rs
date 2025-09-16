mod ast_visit;
mod dynamic_import_to_glob;

use std::{borrow::Cow, path::Path, pin::Pin, sync::Arc};

use derive_more::Debug;
use oxc::{ast::AstBuilder, ast_visit::VisitMut};
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformAstArgs, HookTransformAstReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_utils::{
  futures::{block_on, block_on_spawn_all},
  pattern_filter::{StringOrRegex, filter as pattern_filter},
};
use sugar_path::SugarPath as _;

pub const DYNAMIC_IMPORT_HELPER: &str = "\0rolldown_dynamic_import_helper.js";

pub type ResolverFn = dyn Fn(String, String) -> Pin<Box<(dyn Future<Output = anyhow::Result<Option<String>>> + Send)>>
  + Send
  + Sync;

#[derive(Debug, Default)]
pub struct DynamicImportVarsPlugin {
  pub include: Vec<StringOrRegex>,
  pub exclude: Vec<StringOrRegex>,
  #[debug(skip)]
  pub resolver: Option<Arc<ResolverFn>>,
}

impl DynamicImportVarsPlugin {
  fn filter(&self, id: &str, cwd: &Path) -> bool {
    if self.include.is_empty() && self.exclude.is_empty() {
      return true;
    }

    let exclude = (!self.exclude.is_empty()).then_some(self.exclude.as_slice());
    let include = (!self.include.is_empty()).then_some(self.include.as_slice());
    pattern_filter(exclude, include, id, &cwd.to_string_lossy()).inner()
  }
}

impl Plugin for DynamicImportVarsPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:dynamic-import-vars")
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

  async fn load(&self, _ctx: &PluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    Ok((args.id == DYNAMIC_IMPORT_HELPER).then_some(HookLoadOutput {
      code: arcstr::literal!(include_str!("dynamic-import-helper.js")),
      ..Default::default()
    }))
  }

  async fn transform_ast(
    &self,
    ctx: &PluginContext,
    mut args: HookTransformAstArgs<'_>,
  ) -> HookTransformAstReturn {
    if !self.filter(args.id, args.cwd) {
      return Ok(args.ast);
    }
    args.ast.program.with_mut(|fields| {
      let source_text = fields.source.as_str();
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut visitor = ast_visit::DynamicImportVarsVisit {
        ctx,
        source_text,
        ast_builder,
        root: args.cwd,
        importer: args.id.as_path(),
        need_helper: false,
        comments: &fields.program.comments,
        current_comment: 0,
        async_imports: Vec::default(),
        async_imports_addrs: Vec::default(),
      };

      visitor.visit_statements(&mut fields.program.body);

      if !visitor.async_imports.is_empty()
        && let Some(resolver) = &self.resolver
      {
        let async_imports = std::mem::take(&mut visitor.async_imports);
        let task = async_imports
          .into_iter()
          .map(|glob| async { resolver(glob, args.id.to_string()).await.ok()? });

        let importer = args.id.as_path().parent().unwrap();
        let result = block_on(block_on_spawn_all(task));
        for (i, item) in result.into_iter().enumerate() {
          if let Some(id) = item {
            let id = id.relative(importer);
            let id = id.to_slash_lossy();
            let id = if id.is_empty() {
              continue;
            } else if id.as_bytes()[0] == b'.' {
              id.into_owned()
            } else {
              rolldown_utils::concat_string!("./", id)
            };

            let addr = visitor.async_imports_addrs[i];
            visitor.rewrite_variable_dynamic_import(unsafe { &mut *addr }, Some(&id));
          }
        }
      }

      if visitor.need_helper {
        fields.program.body.push(visitor.variable_dynamic_import_runtime_helper());
      }
    });

    Ok(args.ast)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::TransformAst
  }
}
