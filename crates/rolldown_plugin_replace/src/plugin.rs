use std::{cmp::Reverse, collections::HashMap};

use fancy_regex::Regex;
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, Plugin};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

#[derive(Debug)]
pub struct ReplaceOptions {
  pub values: HashMap</* Target */ String, /* Replacement */ String>,
  /// Default to `("\\b", "\\b(?!\\.)")`. To prevent `typeof window.document` from being replaced by config item `typeof window` => `"object"`.
  pub delimiters: (String, String),
}

impl Default for ReplaceOptions {
  fn default() -> Self {
    Self { values: HashMap::default(), delimiters: ("\\b".to_string(), "\\b(?!\\.)".to_string()) }
  }
}

#[derive(Debug)]
pub struct ReplacePlugin {
  matcher: Regex,
  values: FxHashMap</* Target */ String, /* Replacement */ String>,
}

impl ReplacePlugin {
  pub fn new(values: HashMap<String, String>) -> Self {
    Self::with_options(ReplaceOptions { values, ..Default::default() })
  }

  pub fn with_options(options: ReplaceOptions) -> Self {
    let mut keys = options.values.keys().collect::<Vec<_>>();
    // Sort by length in descending order so that longer targets are matched first.
    keys.sort_by_key(|key| Reverse(key.len()));

    let joined_keys = keys.iter().map(|key| fancy_regex::escape(key)).collect::<Vec<_>>().join("|");
    let (delimiter_left, delimiter_right) = &options.delimiters;
    // https://rustexp.lpil.uk/
    let pattern = format!("{delimiter_left}({joined_keys}){delimiter_right}");
    Self {
      matcher: Regex::new(&pattern).unwrap_or_else(|_| panic!("Invalid regex {pattern:?}")),
      values: options.values.into_iter().collect(),
    }
  }

  fn try_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> bool {
    let mut changed = false;
    for captures in self.matcher.captures_iter(code) {
      let Ok(captures) = captures else {
        continue;
      };
      changed = true;
      let Some(matched) = captures.get(1) else {
        continue;
      };
      let Some(replacement) = self.values.get(matched.as_str()) else {
        continue;
      };
      magic_string.update(matched.start(), matched.end(), replacement);
    }

    changed
  }
}

impl Plugin for ReplacePlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "builtin:replace".into()
  }

  async fn transform(
    &self,
    _ctx: &rolldown_plugin::TransformPluginContext<'_>,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let mut magic_string = MagicString::new(args.code);
    if self.try_replace(args.code, &mut magic_string) {
      return Ok(Some(HookTransformOutput {
        code: Some(magic_string.to_string()),
        ..Default::default()
      }));
    }
    Ok(None)
  }

  async fn render_chunk(
    &self,
    _ctx: &rolldown_plugin::PluginContext,
    args: &rolldown_plugin::HookRenderChunkArgs<'_>,
  ) -> rolldown_plugin::HookRenderChunkReturn {
    let mut magic_string = MagicString::new(&args.code);
    if self.try_replace(&args.code, &mut magic_string) {
      return Ok(Some(HookRenderChunkOutput { code: magic_string.to_string(), map: None }));
    }
    Ok(None)
  }
}
