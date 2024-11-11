use std::path::Path;
use std::sync::{Arc, Mutex};
use std::{borrow::Cow, collections::HashMap};
use sugar_path::SugarPath;

use rolldown_plugin::{
  HookLoadOutput, HookLoadReturn, HookNoopReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, Plugin, PluginContext,
};

pub struct VirtualOption {
  pub modules: HashMap<String, String>,
}

#[derive(Debug, Default)]
pub struct VirtualPlugin {
  modules: HashMap<String, String>,
  resolved_ids: Arc<Mutex<HashMap<String, String>>>,
}

static PREFIX: &str = "\0virtual:";

impl VirtualPlugin {
  pub fn new(option: VirtualOption) -> Self {
    Self { modules: option.modules, resolved_ids: Arc::new(Mutex::new(HashMap::new())) }
  }
}

impl Plugin for VirtualPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:virtual")
  }

  async fn build_start(&self, ctx: &PluginContext) -> HookNoopReturn {
    let resolved_ids: HashMap<_, _> = self
      .modules
      .iter()
      .map(|(id, code)| {
        let buf = ctx.cwd().join(id);
        (buf.normalize().to_string_lossy().into_owned(), code.clone())
      })
      .collect();

    let mut lock = self.resolved_ids.lock().unwrap();
    *lock = resolved_ids;
    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if self.modules.contains_key(args.specifier) {
      return Ok(Some(HookResolveIdOutput {
        id: [PREFIX, args.specifier].concat(),
        external: None,
        side_effects: None,
      }));
    }

    if let Some(importer) = args.importer {
      let importer_no_prefix = importer.strip_prefix(PREFIX).unwrap_or(importer);
      let resolved_path = ctx
        .cwd()
        .join(Path::new(importer_no_prefix).parent().unwrap_or_else(|| Path::new(".")))
        .join(args.specifier)
        .normalize();
      let resolved = resolved_path.to_string_lossy().into_owned();
      let resolved_ids = self.resolved_ids.lock().unwrap();
      if resolved_ids.contains_key(&resolved) {
        return Ok(Some(HookResolveIdOutput {
          id: [PREFIX, &resolved].concat(),
          external: None,
          side_effects: None,
        }));
      }
    }

    Ok(None)
  }

  async fn load(
    &self,
    _ctx: &PluginContext,
    args: &rolldown_plugin::HookLoadArgs<'_>,
  ) -> HookLoadReturn {
    if let Some(id) = args.id.strip_prefix(PREFIX) {
      let code = self.modules.get(id).cloned().or_else(|| {
        let resolved_ids = self.resolved_ids.lock().unwrap();
        resolved_ids.get(id).cloned()
      });
      if let Some(code) = code {
        return Ok(Some(HookLoadOutput { code, map: None, side_effects: None, module_type: None }));
      }
    }
    Ok(None)
  }
}
