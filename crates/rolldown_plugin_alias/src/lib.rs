use std::borrow::Cow;
use std::sync::Arc;

use rolldown_plugin::{
  HookResolveIdOutput, HookUsage, Plugin, PluginContext, PluginContextResolveOptions,
};
use rolldown_utils::pattern_filter::StringOrRegex;

#[derive(Debug, Default)]
pub struct AliasPlugin {
  // We don't support `customResolver` and `resolverFunction`, it will generate many threadSafeFunction in queue, and slowdown the
  // performance, downstream user should fallback to js alias plugin when needs advance feature.
  pub entries: Vec<Alias>,
}

#[derive(Debug)]
pub struct Alias {
  pub find: StringOrRegex,
  pub replacement: String,
}

impl AliasPlugin {
  fn matches(pattern: &StringOrRegex, importee: &str) -> bool {
    match pattern {
      StringOrRegex::String(p) => {
        if importee.len() < p.len() {
          return false;
        }
        if importee == p {
          return true;
        }
        // avoid alloc
        importee.starts_with(p) && importee[p.len()..].starts_with('/')
      }
      StringOrRegex::Regex(regex) => regex.matches(importee),
    }
  }
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
    let match_entry = self.entries.iter().find(|alias| Self::matches(&alias.find, importee));
    let Some(match_entry) = match_entry else {
      return Ok(None);
    };

    let update_id = match &match_entry.find {
      StringOrRegex::String(find) => importee.replace(find, &match_entry.replacement),
      StringOrRegex::Regex(find) => find.replace_all(importee, &match_entry.replacement),
    };
    Ok(
      ctx
        .resolve(
          &update_id,
          None,
          Some(PluginContextResolveOptions {
            import_kind: args.kind,
            skip_self: true,
            custom: Arc::clone(&args.custom),
          }),
        )
        .await?
        .map(|resolved_id| {
          Some(HookResolveIdOutput { id: resolved_id.id, ..Default::default() })
        })?,
    )
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}
