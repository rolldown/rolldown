use std::sync::Arc;
use std::{borrow::Cow, path::Path};

use cow_utils::CowUtils;
use rolldown_plugin::{
  HookResolveIdOutput, HookUsage, Plugin, PluginContext, PluginContextResolveOptions,
};
use rolldown_utils::pattern_filter::StringOrRegex;

#[derive(Debug, Default)]
pub struct AliasPlugin {
  // We don't support `customResolver` and `resolverFunction`, it will generate many threadSafeFunction in queue,
  // and slowdown the performance, downstream user should fallback to js alias plugin when needs advance feature.
  pub entries: Vec<Alias>,
}

#[derive(Debug)]
pub struct Alias {
  pub find: StringOrRegex,
  pub replacement: String,
}

impl Plugin for AliasPlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:alias")
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    let importee = args.specifier;
    let matched_entry = self.entries.iter().find(|alias| matches(&alias.find, importee));

    let Some(matched_entry) = matched_entry else { return Ok(None) };
    let specifier = match &matched_entry.find {
      StringOrRegex::String(find) => importee.cow_replacen(find, &matched_entry.replacement, 1),
      StringOrRegex::Regex(find) => find.replace(importee, &matched_entry.replacement),
    };

    let resolved_id = ctx
      .resolve(
        &specifier,
        args.importer,
        Some(PluginContextResolveOptions {
          skip_self: true,
          import_kind: args.kind,
          is_entry: args.is_entry,
          custom: Arc::clone(&args.custom),
        }),
      )
      .await?;

    // TODO: support `viteAliasCustomResolver`
    // https://github.com/vitejs/rolldown-vite/blob/91a494c/packages/vite/src/node/plugins/index.ts#L325-L334

    Ok(Some(match resolved_id {
      Ok(resolved_id) => HookResolveIdOutput::from_resolved_id(resolved_id),
      Err(_) => {
        if !Path::new(specifier.as_ref()).is_absolute() {
          let message = format!(
            "rewrote {importee} to {specifier} but was not an absolute path and was not handled by other plugins. This will lead to duplicated modules for the same path. To avoid duplicating modules, you should resolve to an absolute path."
          );
          ctx.warn(rolldown_plugin::Log { message, code: None, id: None, exporter: None });
        }
        HookResolveIdOutput::from_id(specifier)
      }
    }))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

fn matches(pattern: &StringOrRegex, importee: &str) -> bool {
  match pattern {
    StringOrRegex::String(p) => {
      if importee.len() < p.len() {
        return false;
      }
      importee == p || (importee.starts_with(p) && importee.as_bytes()[p.len()] == b'/')
    }
    StringOrRegex::Regex(regex) => regex.matches(importee),
  }
}
