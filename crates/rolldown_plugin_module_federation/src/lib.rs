use std::borrow::Cow;

mod init_modules_visitor;
mod option;
mod utils;
pub use option::{ModuleFederationPluginOption, Remote, Shared};
use oxc::{
  ast::{
    ast::{ImportOrExportKind, Statement},
    AstBuilder, VisitMut, NONE,
  },
  span::SPAN,
};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookResolveIdReturn, Plugin};
use rolldown_utils::concat_string;
use utils::is_remote_module;

const REMOTE_ENTRY: &str = "mf:remote-entry.js";
const INIT_HOST: &str = "mf:init-host.js";

#[derive(Debug)]
pub struct ModuleFederationPlugin {
  options: ModuleFederationPluginOption,
}

impl ModuleFederationPlugin {
  pub fn new(options: ModuleFederationPluginOption) -> Self {
    Self { options }
  }

  pub fn generate_remote_entry_code(&self) -> String {
    let expose = self
      .options
      .exposes
      .as_ref()
      .map(|exposes| {
        exposes
          .iter()
          .map(|(key, value)| concat_string!("'", key, "': () => import('", value, "')"))
          .collect::<Vec<_>>()
          .join(", ")
      })
      .unwrap_or_default();
    include_str!("remote-entry.js")
      .replace("__EXPOSES_MAP__", &concat_string!("{", expose, "}"))
      .to_string()
  }

  pub fn generate_init_host_code(&self) -> String {
    let remotes = self
      .options
      .remotes
      .as_ref()
      .map(|remotes| {
        remotes
          .iter()
          .map(|value| {
            concat_string!(
              "{ entryGlobalName: '",
              value.entry_global_name.as_deref().unwrap_or_else(|| &value.name),
              "', name: '",
              value.name,
              "', entry: '",
              value.entry,
              "', type: '",
              value.r#type.as_deref().unwrap_or("var"),
              "' }"
            )
          })
          .collect::<Vec<_>>()
          .join(", ")
      })
      .unwrap_or_default();
    include_str!("init-host.js")
      .replace("__REMOTES__", &concat_string!("[", remotes, "]"))
      .to_string()
  }
}

impl Plugin for ModuleFederationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:module-federation")
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.options.exposes.is_some() {
      ctx
        .emit_chunk(EmittedChunk {
          file_name: Some(
            self.options.filename.as_deref().expect("The expose filename is required").into(),
          ),
          id: REMOTE_ENTRY.to_string(),
          ..Default::default()
        })
        .await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == REMOTE_ENTRY
      || args.specifier == INIT_HOST
      || is_remote_module(args.specifier, &self.options)
    {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }
    if let Some(remotes) = self.options.remotes.as_ref() {
      for remote in remotes {
        if args.specifier.starts_with(&remote.name) {
          return Ok(Some(rolldown_plugin::HookResolveIdOutput {
            id: args.specifier.to_string(),
            ..Default::default()
          }));
        }
      }
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id == REMOTE_ENTRY {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: self.generate_remote_entry_code(),
        ..Default::default()
      }));
    }
    if args.id == INIT_HOST {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: self.generate_init_host_code(),
        ..Default::default()
      }));
    }
    if is_remote_module(args.id, &self.options) {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: include_str!("remote-module.js")
          .replace("__REMOTE__MODULE__ID__", args.id)
          .to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if args.is_user_defined_entry && self.options.remotes.is_some() {
      args.ast.program.with_mut(|fields| {
        let ast_builder = AstBuilder::new(fields.allocator);
        fields.program.body.insert(
          0,
          Statement::from(ast_builder.module_declaration_import_declaration(
            SPAN,
            None,
            ast_builder.string_literal(SPAN, ast_builder.atom(INIT_HOST), None),
            None,
            NONE,
            ImportOrExportKind::Value,
          )),
        );
      });
    }

    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut init_modules_visitor = init_modules_visitor::InitModuleVisitor {
        ast_builder,
        options: &self.options,
        statements: vec![],
      };
      init_modules_visitor.visit_program(fields.program);
      let old_body = fields.program.body.drain(..).collect::<Vec<_>>();
      fields.program.body.extend(init_modules_visitor.statements);
      fields.program.body.extend(old_body);
    });

    Ok(args.ast)
  }
}
