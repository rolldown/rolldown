use anyhow::Ok;
use oxc::codegen::{Codegen, CodegenOptions};
use oxc::isolated_declarations::{IsolatedDeclarations, IsolatedDeclarationsOptions};
use rolldown_common::ModuleType;
use rolldown_plugin::{
  HookLoadOutput, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_plugin::{PluginHookMeta, PluginOrder};
use rolldown_utils::dashmap::FxDashMap;
use std::{borrow::Cow, path::Path};
use sugar_path::SugarPath;

mod type_import_visitor;

#[derive(Debug, Default)]
pub struct IsolatedDeclarationPlugin {
  pub strip_internal: bool,
  dts_map: FxDashMap<String, String>,
}

impl IsolatedDeclarationPlugin {
  pub fn new(strip_internal: bool) -> Self {
    Self { strip_internal, dts_map: FxDashMap::default() }
  }
}

impl Plugin for IsolatedDeclarationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:isolated-declarations")
  }

  async fn transform_ast(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: rolldown_plugin::HookTransformAstArgs<'_>,
  ) -> rolldown_plugin::HookTransformAstReturn {
    if !matches!(args.module_type, ModuleType::Ts | ModuleType::Tsx) || is_in_node_modules(args.id)
    {
      return Ok(args.ast);
    }

    let module_info = ctx.get_module_info(args.id);
    let dts_id = filename_to_dts(args.id);

    let mut ast = args.ast.clone_with_another_arena();
    let ret = ast.program.with_mut(|fields| {
      IsolatedDeclarations::new(
        fields.allocator,
        IsolatedDeclarationsOptions { strip_internal: self.strip_internal },
      )
      .build(fields.program)
    });

    // TODO BuildDiagnostic error
    if !ret.errors.is_empty() {
      return Err(anyhow::anyhow!("IsolatedDeclarations error"));
    }

    let codegen_ret = Codegen::new().with_options(CodegenOptions::default()).build(&ret.program);
    self.dts_map.insert(dts_id.clone(), codegen_ret.code);

    let is_entry = module_info.map_or(false, |info| info.is_entry);
    if is_entry {
      ctx
        .emit_chunk(rolldown_common::EmittedChunk {
          id: dts_id,
          name: None, // todo!,
          file_name: None,
          importer: None,
        })
        .await?;
    }

    Ok(args.ast)
  }

  // The rolldown strip types at the end of the build process, make sure to run this plugin before that.
  fn transform_ast_meta(&self) -> Option<PluginHookMeta> {
    Some(PluginHookMeta { order: Some(PluginOrder::Pre) })
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if self.dts_map.contains_key(args.specifier) {
      return Ok(Some(HookResolveIdOutput { id: args.specifier.into(), ..Default::default() }));
    }
    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> rolldown_plugin::HookLoadReturn {
    if !self.dts_map.contains_key(args.id) {
      return Ok(None);
    }

    let code = self.dts_map.get(args.id).unwrap();
    Ok(Some(HookLoadOutput {
      code: code.to_string(),
      module_type: Some(ModuleType::Dts),
      ..Default::default()
    }))
  }
  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::TransformAst | HookUsage::Load | HookUsage::ResolveId
  }
}

fn is_in_node_modules(id: &str) -> bool {
  id.replace("\\", "/").contains("/node_modules/")
}

// fn is_ts_filename(id: &str) -> bool {
//   let ext = Path::new(id).extension().and_then(|s| s.to_str()).unwrap_or_default();
//   matches!(ext, "ts" | "tsx" | "mts" | "cts")
// }

fn filename_to_dts(id: &str) -> String {
  let mut path = Path::new(id).to_path_buf();
  let prefix = match path.extension().and_then(|s| s.to_str()).unwrap_or_default() {
    "cjs" | "cts" => "c",
    "mjs" | "mts" => "m",
    _ => "",
  };
  path.set_extension(format!("d.{}ts", prefix));
  path.to_slash_lossy().into()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_filename_ts_to_dts() {
    assert_eq!(filename_to_dts("a.ts"), "a.d.ts");
    assert_eq!(filename_to_dts("a.cts"), "a.d.cts");
    assert_eq!(filename_to_dts("a.mts"), "a.d.mts");
    assert_eq!(filename_to_dts("a.tsx"), "a.d.ts");
    assert_eq!(filename_to_dts("a"), "a.d.ts");

    assert_eq!(filename_to_dts("/some/a.ts"), "/some/a.d.ts");
  }
}
