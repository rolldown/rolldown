use std::borrow::Cow;

use rolldown_common::{ImportKind, ResolvedExternal, is_existing_node_builtin_modules};
use rolldown_plugin::{HookLoadOutput, HookResolveIdOutput, HookUsage, Plugin};
use rolldown_utils::{concat_string, pattern_filter::StringOrRegex, rustc_hash::FxHashSetExt as _};
use rustc_hash::FxHashSet;

const CJS_EXTERNAL_FACADE_PREFIX: &str = "builtin:esm-external-require-";

#[derive(Debug, Default)]
pub struct EsmExternalRequirePlugin {
  pub external: Vec<StringOrRegex>,
  pub skip_duplicate_check: bool,
}

impl Plugin for EsmExternalRequirePlugin {
  fn name(&self) -> Cow<'static, str> {
    Cow::Borrowed("builtin:esm-external-require")
  }

  fn register_hook_usage(&self) -> HookUsage {
    if self.external.is_empty() {
      HookUsage::empty()
    } else if self.skip_duplicate_check {
      HookUsage::ResolveId | HookUsage::Load
    } else {
      HookUsage::BuildStart | HookUsage::ResolveId | HookUsage::Load
    }
  }

  async fn build_start(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    if let rolldown_common::IsExternal::StringOrRegex(ref option_externals) = args.options.external
    {
      #[derive(PartialEq, Eq, Hash)]
      enum StringOrRegexPattern<'a> {
        String(&'a str),
        Regex(&'a str),
      }

      let mut externals = FxHashSet::with_capacity(option_externals.len());
      for external in option_externals {
        match external {
          StringOrRegex::String(s) => {
            externals.insert(StringOrRegexPattern::String(s.as_str()));
          }
          StringOrRegex::Regex(r) => {
            if let Some(pattern) = r.regex_pattern() {
              externals.insert(StringOrRegexPattern::Regex(pattern));
            }
          }
        }
      }

      let mut duplicates = Vec::with_capacity(self.external.len().min(option_externals.len()));
      for plugin_external in &self.external {
        match plugin_external {
          StringOrRegex::String(s) => {
            if externals.contains(&StringOrRegexPattern::String(s)) {
              duplicates.push(s.as_str());
            }
          }
          StringOrRegex::Regex(r) => {
            if let Some(pattern) = r.regex_pattern()
              && externals.contains(&StringOrRegexPattern::Regex(pattern))
            {
              duplicates.push(pattern);
            }
          }
        }
      }

      if !duplicates.is_empty() {
        ctx.warn(rolldown_plugin::LogWithoutPlugin {
          message: format!(
            "Found {} duplicate external: `{}`. Remove them from top-level `external` as they're already handled by `{}` plugin. To disable this check, set `skipDuplicateCheck: true`.",
            duplicates.len(),
            duplicates.join("`, `"),
            self.name()
          ),
          ..Default::default()
        });
      }
    }

    Ok(())
  }

  async fn resolve_id(
    &self,
    ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookResolveIdArgs<'_>,
  ) -> rolldown_plugin::HookResolveIdReturn {
    if args.importer.is_some_and(|importer| importer.starts_with(CJS_EXTERNAL_FACADE_PREFIX)) {
      return Ok(Some(HookResolveIdOutput {
        id: args.specifier.into(),
        external: Some(ResolvedExternal::Bool(true)),
        ..Default::default()
      }));
    }

    let is_external = self.external.iter().any(|v| match v {
      StringOrRegex::String(string) => string == args.specifier,
      StringOrRegex::Regex(regex) => regex.matches(args.specifier),
    });

    if is_external {
      if !ctx.options().format.is_esm() || args.kind != ImportKind::Require {
        return Ok(Some(HookResolveIdOutput {
          id: args.specifier.into(),
          external: Some(ResolvedExternal::Bool(true)),
          ..Default::default()
        }));
      }

      return Ok(Some(HookResolveIdOutput {
        id: concat_string!(CJS_EXTERNAL_FACADE_PREFIX, args.specifier).into(),
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
    Ok(args.id.strip_prefix(CJS_EXTERNAL_FACADE_PREFIX).map(|module_id| {
      let code = concat_string!(
        "import * as m from '",
        module_id,
        "';module.exports = ",
        if is_existing_node_builtin_modules(module_id) { "m.default" } else { "{ ...m }" },
        ";"
      );
      HookLoadOutput { code: code.into(), ..Default::default() }
    }))
  }
}
