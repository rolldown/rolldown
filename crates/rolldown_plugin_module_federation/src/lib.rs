use std::{
  borrow::Cow,
  sync::atomic::{AtomicBool, Ordering},
};

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
  remote_entry_added: AtomicBool,
}

impl ModuleFederationPlugin {
  pub fn new(options: ModuleFederationPluginOption) -> Self {
    Self { options, remote_entry_added: AtomicBool::default() }
  }
}

impl Plugin for ModuleFederationPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:module-federation")
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    if !self.remote_entry_added.load(Ordering::Relaxed) {
      self.remote_entry_added.store(true, Ordering::Relaxed);
      if self.options.exposes.is_some() {
        let r = ctx
          .emit_chunk(EmittedChunk {
            file_name: Some(
              self.options.filename.as_deref().expect("The expose filename is required").into(),
            ),
            id: REMOTE_ENTRY.to_string(),
            ..Default::default()
          })
          .await;
        dbg!(r);
      }
    }
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
    println!("111 exposes: {:?}", args.id);
    if args.id == REMOTE_ENTRY {
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
      return Ok(Some(rolldown_plugin::HookLoadOutput {
        code: include_str!("remote-entry.js")
          .replace("__EXPOSES_MAP__", &concat_string!("{", expose, "}"))
          .to_string(),
        ..Default::default()
      }));
    }
    Ok(None)
  }
}
