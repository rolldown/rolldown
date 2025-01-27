use std::{borrow::Cow, sync::Arc};

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
use rolldown_utils::{concat_string, dashmap::FxDashMap};
use rustc_hash::FxHashSet;
use utils::is_remote_module;

const REMOTE_ENTRY: &str = "mf:remote-entry.js";
const INIT_HOST: &str = "mf:init-host.js";
const REMOTE_MODULE_REGISTRY: &str = "mf:remote-module-registry.js";
const INIT_MODULE_PREFIX: &str = "mf:init-module:";

#[derive(Debug)]
pub struct ModuleFederationPlugin {
  options: ModuleFederationPluginOption,
  module_init_remote_modules: FxDashMap<Arc<str>, FxHashSet<Arc<str>>>,
}

impl ModuleFederationPlugin {
  pub fn new(options: ModuleFederationPluginOption) -> Self {
    Self { options, module_init_remote_modules: FxDashMap::default() }
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
      .replace("__PLUGINS__", &self.generate_runtime_plugins())
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
      .replace("__PLUGINS__", &self.generate_runtime_plugins())
      .to_string()
  }

  pub fn generate_runtime_plugins(&self) -> String {
    let (plugin_imports, plugin_names) = self
      .options
      .runtime_plugins
      .as_ref()
      .map(|plugins| {
        let mut plugin_imports = Vec::with_capacity(plugins.capacity());
        let mut plugin_names = Vec::with_capacity(plugins.capacity());
        for (index, plugin) in plugins.iter().enumerate() {
          let plugin_name = format!("plugin{index}");
          plugin_imports.push(concat_string!("import ", plugin_name, " from '", plugin, "';"));
          plugin_names.push(concat_string!(plugin_name, "()"));
        }
        (plugin_imports.join("\n"), plugin_names.join(", "))
      })
      .unwrap_or_default();
    concat_string!(plugin_imports, "const plugins = [", plugin_names, "];")
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
    if let Some(filename) = self.options.filename.as_deref() {
      ctx
        .emit_chunk(EmittedChunk {
          file_name: Some(filename.into()),
          id: REMOTE_ENTRY.to_string(),
          ..Default::default()
        })
        .await?;
    }
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == REMOTE_ENTRY
      || args.specifier == INIT_HOST
      || args.specifier == REMOTE_MODULE_REGISTRY
      || is_remote_module(args.specifier, &self.options)
      || args.specifier.starts_with(INIT_MODULE_PREFIX)
    {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.to_string(),
        ..Default::default()
      }));
    }
    if args.specifier == "@module-federation/runtime" {
      let resolve_id = ctx.resolve(args.specifier, None, None).await??;
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: resolve_id.id.to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if args.id == REMOTE_ENTRY && self.options.filename.is_some() {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: self.generate_remote_entry_code(),
        ..Default::default()
      }));
    }
    if args.id == INIT_HOST && self.options.filename.is_none() {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: self.generate_init_host_code(),
        ..Default::default()
      }));
    }
    if args.id == REMOTE_MODULE_REGISTRY {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: include_str!("remote-module-registry.js").to_string(),
        ..Default::default()
      }));
    }
    if args.id.starts_with(INIT_MODULE_PREFIX) {
      let init_remote_modules = self
        .module_init_remote_modules
        .get(&Arc::from(args.id))
        .expect("should have init remote modules");
      let modules_string = concat_string!(
        "[",
        init_remote_modules
          .iter()
          .map(|m| concat_string!("'", m.as_ref(), "'"))
          .collect::<Vec<_>>()
          .join(", "),
        "]"
      );
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: include_str!("init-module-import-remote-module.js")
          .replace("__REMOTE__MODULES__", &modules_string)
          .to_string(),
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
    args.ast.program.with_mut(|fields| {
      let ast_builder = AstBuilder::new(fields.allocator);
      let mut init_remote_modules = FxHashSet::default();
      let mut init_modules_visitor = init_modules_visitor::InitModuleVisitor {
        ast_builder,
        options: &self.options,
        init_remote_modules: &mut init_remote_modules,
      };
      init_modules_visitor.visit_program(fields.program);
      if !init_remote_modules.is_empty() {
        let id = concat_string!(INIT_MODULE_PREFIX, args.id);
        fields.program.body.insert(
          0,
          Statement::from(ast_builder.module_declaration_import_declaration(
            SPAN,
            None,
            ast_builder.string_literal(SPAN, ast_builder.atom(&id), None),
            None,
            NONE,
            ImportOrExportKind::Value,
          )),
        );
        self.module_init_remote_modules.insert(id.into(), init_remote_modules);
      }
    });

    // Init host should be added to the top of the entry file
    if args.is_user_defined_entry && self.options.filename.is_none() && args.id != REMOTE_ENTRY {
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

    Ok(args.ast)
  }
}
