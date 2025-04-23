use std::borrow::Cow;
use std::sync::Arc;

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
    let update_id = match &matched_entry.find {
      StringOrRegex::String(find) => importee.replace(find, &matched_entry.replacement),
      StringOrRegex::Regex(find) => find.replace_all(importee, &matched_entry.replacement),
    };

    let resolved_id = ctx
      .resolve(
        &update_id,
        args.importer,
        Some(PluginContextResolveOptions {
          skip_self: true,
          import_kind: args.kind,
          custom: Arc::clone(&args.custom),
        }),
      )
      .await??;

    // TODO: give an warning
    // if !Path::new(&update_id).is_absolute() {
    //   this.warn(
    //     `rewrote ${importee} to ${updatedId} but was not an absolute path and was not handled by other plugins. ` +
    //       `This will lead to duplicated modules for the same path. ` +
    //       `To avoid duplicating modules, you should resolve to an absolute path.`
    //   );
    // }

    // TODO: support `viteAliasCustomResolver`
    // https://github.com/vitejs/rolldown-vite/blob/91a494c/packages/vite/src/node/plugins/index.ts#L325-L334
    Ok(Some(HookResolveIdOutput { id: resolved_id.id, ..Default::default() }))
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
