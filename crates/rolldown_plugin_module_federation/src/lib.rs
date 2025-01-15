use std::borrow::Cow;

mod option;
pub use option::{ModuleFederationPluginOption, Remote, Shared};
use rolldown_common::EmittedChunk;
use rolldown_plugin::{HookResolveIdReturn, Plugin};
use rolldown_utils::concat_string;

const REMOTE_ENTRY: &str = "mf:remote-entry.js";

#[derive(Debug)]
pub struct ModuleFederationPlugin {
  #[allow(dead_code)]
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
    if args.specifier == REMOTE_ENTRY {
      return Ok(Some(rolldown_plugin::HookResolveIdOutput {
        id: REMOTE_ENTRY.to_string(),
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
    if args.id == REMOTE_ENTRY {
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: self.generate_remote_entry_code(),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}
