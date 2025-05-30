use std::borrow::Cow;

mod init_modules_visitor;
mod manifest;
mod option;
mod utils;
use arcstr::ArcStr;
use cow_utils::CowUtils;
use manifest::generate_manifest;
pub use option::{Manifest, ModuleFederationPluginOption, Remote, Shared};
use oxc::{
  ast::{
    AstBuilder, NONE,
    ast::{ImportOrExportKind, Statement},
  },
  ast_visit::VisitMut,
  span::SPAN,
};
use rolldown_common::{EmittedChunk, Output};
use rolldown_plugin::{HookResolveIdReturn, HookUsage, Plugin};
use rolldown_utils::{concat_string, dashmap::FxDashMap};
use rustc_hash::FxHashSet;
use utils::{
  ResolvedRemoteModule, detect_remote_module_type, generate_remote_module_is_cjs_placeholder,
  get_remote_module_prefix,
};

const REMOTE_ENTRY: &str = "mf:remote-entry.js";
const INIT_HOST: &str = "mf:init-host.js";
const REMOTE_MODULE_REGISTRY: &str = "mf:remote-module-registry.js";
const INIT_REMOTE_MODULE_PREFIX: &str = "mf:init-remote-module:";
const INIT_SHARED_MODULE_PREFIX: &str = "mf:init-shared-module:";
const SHARED_MODULE_PREFIX: &str = "mf:shared-module:";

#[derive(Debug)]
pub struct ModuleFederationPlugin {
  options: ModuleFederationPluginOption,
  resolved_remote_modules: FxDashMap<ArcStr, ResolvedRemoteModule>,
  resolved_id_to_remote_module_keys: FxDashMap<ArcStr, ArcStr>,
}

impl ModuleFederationPlugin {
  pub fn new(options: ModuleFederationPluginOption) -> Self {
    Self {
      options,
      resolved_remote_modules: FxDashMap::default(),
      resolved_id_to_remote_module_keys: FxDashMap::default(),
    }
  }

  pub fn generate_load_remote_module_code(
    &self,
    remote_module_key: &str,
    import_module: &str,
    is_shared: bool,
  ) -> String {
    let remote_module = self
      .resolved_remote_modules
      .get(remote_module_key)
      .expect("remote module should have resolved");
    // The remote module is commonjs, it will be generate `export default require_xx()`, here need to return imported default.
    // The remote module is esm, need to add `__esModule` flag to make sure interop is correct.
    concat_string!(
      "const module = await import('",
      import_module,
      "');",
      "if (",
      remote_module.value().placeholder,
      ") return ",
      if is_shared { "() => " } else { "" },
      "module.default;",
      "const target = { ...module };",
      "Object.defineProperty(target, '__esModule', { value: true, enumerable: false });",
      "return ",
      if is_shared { "() => " } else { "" },
      "target;"
    )
  }

  pub fn generate_remote_entry_code(&self) -> ArcStr {
    let expose = self
      .options
      .exposes
      .as_ref()
      .map(|exposes| {
        exposes
          .iter()
          .map(|(key, value)| {
            concat_string!(
              "'",
              key,
              "': async () => {",
              self.generate_load_remote_module_code(key, value, false),
              "}"
            )
          })
          .collect::<Vec<_>>()
          .join(", ")
      })
      .unwrap_or_default();
    ArcStr::from(
      include_str!("runtime/remote-entry.js")
        .cow_replace("__EXPOSES_MAP__", &concat_string!("{", expose, "}"))
        .cow_replace("__PLUGINS__", &self.generate_runtime_plugins())
        .cow_replace("__SHARED__", &self.generate_shared_modules())
        .cow_replace("__NAME__", &concat_string!("'", &self.options.name, "'")),
    )
  }

  pub fn generate_init_host_code(&self) -> ArcStr {
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
    ArcStr::from(
      include_str!("runtime/init-host.js")
        .cow_replace("__REMOTES__", &concat_string!("[", remotes, "]"))
        .cow_replace("__PLUGINS__", &self.generate_runtime_plugins())
        .cow_replace("__NAME__", &concat_string!("'", &self.options.name, "'"))
        .cow_replace("__SHARED__", &self.generate_shared_modules()),
    )
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

  pub fn generate_shared_modules(&self) -> String {
    let shared = self
      .options
      .shared
      .as_ref()
      .map(|shared| {
        shared
          .iter()
          .map(|(key, value)| {
            concat_string!(
              "'",
              key,
              "': { version: '",
              self.resolved_remote_modules.get(key.as_str()).map_or_else(
                || { value.version.as_deref().unwrap_or_default().into() },
                |v| v.value().version.as_deref().map(ToString::to_string).unwrap_or_default()
              ),
              "', scope: ['",
              value.share_scope.as_deref().unwrap_or("default"),
              "'], from: '",
              self.options.name.as_str(),
              "', async get() {",
              self.generate_load_remote_module_code(
                key,
                &concat_string!(SHARED_MODULE_PREFIX, key),
                true
              ),
              "}, shareConfig: {",
              value.singleton.map(|v| if v { "singleton: true," } else { "" }).unwrap_or_default(),
              value
                .required_version
                .as_deref()
                .map(|v| concat_string!("requiredVersion: '", v, "',"))
                .unwrap_or_default(),
              value
                .strict_version
                .map(|v| if v { "strictVersion: true," } else { "" })
                .unwrap_or_default(),
              "}}"
            )
          })
          .collect::<Vec<_>>()
          .join(", ")
      })
      .unwrap_or_default();
    concat_string!("{", shared, "};")
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
    } else {
      ctx.emit_chunk(EmittedChunk { id: INIT_HOST.to_string(), ..Default::default() }).await?;
    }

    if let Some(shared) = self.options.shared.as_ref() {
      for (key, item) in shared {
        if item.version.is_none() {
          let resolve_id = ctx.resolve(key.as_str(), None, None).await??;
          self.resolved_remote_modules.insert(
            key.as_str().into(),
            ResolvedRemoteModule {
              id: resolve_id.id.clone(),
              version: resolve_id.package_json.as_ref().and_then(|j| j.version.clone()),
              placeholder: generate_remote_module_is_cjs_placeholder(key.as_str()),
              ..Default::default()
            },
          );
          self.resolved_id_to_remote_module_keys.insert(resolve_id.id.clone(), key.as_str().into());
        }
      }
    }

    if let Some(exposes) = self.options.exposes.as_ref() {
      for (key, value) in exposes {
        let resolve_id = ctx.resolve(value.as_str(), None, None).await??;
        self.resolved_remote_modules.insert(
          key.as_str().into(),
          ResolvedRemoteModule {
            id: resolve_id.id.clone(),
            placeholder: generate_remote_module_is_cjs_placeholder(key.as_str()),
            ..Default::default()
          },
        );
        self.resolved_id_to_remote_module_keys.insert(resolve_id.id.clone(), key.as_str().into());
      }
    }

    Ok(())
  }

  // Make sure the plugin resolve_id hook called at first, avoid vite resolver resolved the shared module.
  fn resolve_id_meta(&self) -> Option<rolldown_plugin::PluginHookMeta> {
    Some(rolldown_plugin::PluginHookMeta { order: Some(rolldown_plugin::PluginOrder::Pre) })
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if args.specifier == REMOTE_ENTRY
      || args.specifier == INIT_HOST
      || args.specifier == REMOTE_MODULE_REGISTRY
      || detect_remote_module_type(args.specifier, &self.options).is_some()
      || args.specifier.starts_with(INIT_REMOTE_MODULE_PREFIX)
      || args.specifier.starts_with(INIT_SHARED_MODULE_PREFIX)
    {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: args.specifier.into(),
        ..Default::default()
      }));
    }
    if args.specifier == "@module-federation/runtime" {
      let resolve_id = ctx.resolve(args.specifier, None, None).await??;
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: resolve_id.id,
        ..Default::default()
      }));
    }
    if args.specifier.starts_with(SHARED_MODULE_PREFIX) {
      let resolve_id =
        ctx.resolve(&args.specifier[SHARED_MODULE_PREFIX.len()..], None, None).await??;
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: resolve_id.id,
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
        code: arcstr::literal!(include_str!("runtime/remote-module-registry.js")),
        ..Default::default()
      }));
    }
    if args.id.starts_with(INIT_REMOTE_MODULE_PREFIX) {
      let remote_module_id = &args.id[INIT_REMOTE_MODULE_PREFIX.len()..];
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: ArcStr::from(
          include_str!("runtime/init-remote-module.js")
            .cow_replace("__MODULE_ID__", &concat_string!("'", remote_module_id, "'"))
            .cow_replace("__IS__SHARED__", "false"),
        ),
        ..Default::default()
      }));
    }
    if args.id.starts_with(INIT_SHARED_MODULE_PREFIX) {
      let remote_module_id = &args.id[INIT_SHARED_MODULE_PREFIX.len()..];
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: ArcStr::from(
          include_str!("runtime/init-remote-module.js")
            .cow_replace("__MODULE_ID__", &concat_string!("'", remote_module_id, "'"))
            .cow_replace("__IS__SHARED__", "true"),
        ),
        ..Default::default()
      }));
    }
    if detect_remote_module_type(args.id, &self.options).is_some() {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: ArcStr::from(
          include_str!("runtime/remote-module.js").cow_replace("__REMOTE__MODULE__ID__", args.id),
        ),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn transform_ast(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    mut args: rolldown_plugin::HookTransformAstArgs<'_>,
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
        let statements = init_remote_modules.iter().map(|remote_module| {
          let id = concat_string!(get_remote_module_prefix(remote_module.r#type), remote_module.id);
          Statement::from(ast_builder.module_declaration_import_declaration(
            SPAN,
            None,
            ast_builder.string_literal(SPAN, ast_builder.atom(&id), None),
            None,
            NONE,
            ImportOrExportKind::Value,
          ))
        });
        let old_body = fields.program.body.drain(..).collect::<Vec<_>>();
        fields.program.body.extend(statements);
        fields.program.body.extend(old_body);
      }

      if args.is_user_defined_entry && self.options.filename.is_none() {
        let old_body = fields.program.body.drain(..).collect::<Vec<_>>();
        fields.program.body.push(Statement::from(
          ast_builder.module_declaration_import_declaration(
            SPAN,
            None,
            ast_builder.string_literal(SPAN, ast_builder.atom(INIT_HOST), None),
            None,
            NONE,
            ImportOrExportKind::Value,
          ),
        ));
        fields.program.body.extend(old_body);
      }
    });

    Ok(args.ast)
  }

  async fn module_parsed(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    _module_info: std::sync::Arc<rolldown_common::ModuleInfo>,
    normal_module: &rolldown_common::NormalModule,
  ) -> rolldown_plugin::HookNoopReturn {
    if let Some(resolved_id_to_remote_module_key) =
      self.resolved_id_to_remote_module_keys.get(normal_module.id.as_ref())
    {
      if let Some(mut remote_module) =
        self.resolved_remote_modules.get_mut(resolved_id_to_remote_module_key.value())
      {
        remote_module.value_mut().is_cjs = normal_module.ecma_view.exports_kind.is_commonjs();
      }
    }
    Ok(())
  }

  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    if args.chunk.facade_module_id.as_deref() == Some(REMOTE_ENTRY)
      || args.chunk.module_ids.iter().any(|module_id| module_id.as_ref() == INIT_HOST)
    {
      let mut code = args.code.clone();
      for remote_module_ref in &self.resolved_remote_modules {
        let remote_module = remote_module_ref.value();
        code = code
          .replace(&remote_module.placeholder, if remote_module.is_cjs { "true" } else { "false" });
      }
      return Ok(Some(rolldown_plugin::HookRenderChunkOutput { code, map: None }));
    }
    Ok(None)
  }

  async fn generate_bundle(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &mut rolldown_plugin::HookGenerateBundleArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if self.options.filename.is_none() {
      // Remove unused INIT HOST entry chunk.
      if let Some(index) = args.bundle.iter().enumerate().find_map(|(index, chunk)| {
        if let Output::Chunk(chunk) = chunk {
          if chunk.is_entry && chunk.facade_module_id.as_deref() == Some(INIT_HOST) {
            return Some(index);
          }
        }
        None
      }) {
        args.bundle.remove(index);
      }
    }
    if self.options.manifest.is_some() && self.options.filename.is_some() {
      generate_manifest(ctx, args, &self.options, &self.resolved_remote_modules).await?;
    }
    Ok(())
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
      | HookUsage::ResolveId
      | HookUsage::Load
      | HookUsage::TransformAst
      | HookUsage::ModuleParsed
      | HookUsage::RenderChunk
      | HookUsage::GenerateBundle
  }
}
