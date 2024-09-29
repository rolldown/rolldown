use std::ops::Range;
use std::{cmp::Reverse, collections::HashMap, sync::LazyLock};

// use fancy_regex::Regex;
use regex::Regex;
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, Plugin};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::utils::expand_typeof_replacements;

#[derive(Debug, Default)]
pub struct ReplaceOptions {
  pub values: HashMap</* Target */ String, /* Replacement */ String>,
  /// Default to `("\\b", "\\b(?!\\.)")`. To prevent `typeof window.document` from being replaced by config item `typeof window` => `"object"`.
  pub delimiters: Option<(String, String)>,
  pub prevent_assignment: bool,
  pub object_guards: bool,
}

// We don't reuse `HybridRegex` in `rolldown_utils`, since
// only the enum is needed
#[derive(Debug)]
enum HybridRegex {
  Optimize(regex::Regex),
  Ecma(regress::Regex),
}

#[derive(Debug)]
pub struct ReplacePlugin {
  matcher: HybridRegex,
  prevent_assignment: bool,
  values: FxHashMap</* Target */ String, /* Replacement */ String>,
}

static NON_ASSIGNMENT_MATCHER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new("\\b(?:const|let|var)\\s+$").expect("Should be valid regex"));

impl ReplacePlugin {
  pub fn new(values: HashMap<String, String>) -> Self {
    Self::with_options(ReplaceOptions { values, ..Default::default() })
  }

  pub fn with_options(options: ReplaceOptions) -> Self {
    let values = if options.object_guards {
      expand_typeof_replacements(&options.values).into_iter().chain(options.values).collect()
    } else {
      options.values
    };
    let mut keys = values.keys().collect::<Vec<_>>();
    // Sort by length in descending order so that longer targets are matched first.
    keys.sort_by_key(|key| Reverse(key.len()));

    let lookahead = if options.prevent_assignment { "(?!\\s*=[^=])" } else { "" };

    let joined_keys = keys.iter().map(|key| regex::escape(key)).collect::<Vec<_>>().join("|");
    // https://rustexp.lpil.uk/
    let matcher = if let Some((delimiter_left, delimiter_right)) = options.delimiters {
      let pattern = format!("{delimiter_left}({joined_keys}){delimiter_right}{lookahead}");
      HybridRegex::Ecma(regress::Regex::new(&pattern).unwrap())
    } else {
      HybridRegex::Optimize(regex::Regex::new(&format!("\\b({joined_keys})\\b")).unwrap())
    };
    Self {
      matcher,
      prevent_assignment: options.prevent_assignment,
      values: values.into_iter().collect(),
    }
  }

  fn try_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> bool {
    match self.matcher {
      HybridRegex::Optimize(ref regex) => self.optimized_replace(code, magic_string, regex),
      HybridRegex::Ecma(ref regex) => self.fallback_replace(code, magic_string, regex),
    }
  }

  fn optimized_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
    regex: &regex::Regex,
  ) -> bool {
    let mut changed = false;
    for captures in regex.captures_iter(code) {
      let Some(matched) = captures.get(1) else {
        break;
      };
      if self.look_around_assert(code, matched.range()) {
        continue;
      }
      let Some(replacement) = self.values.get(matched.as_str()) else {
        break;
      };
      changed = true;
      magic_string.update(matched.start(), matched.end(), replacement);
    }

    changed
  }

  fn look_around_assert(&self, code: &str, matched_range: Range<usize>) -> bool {
    if self.prevent_assignment {
      let before = &code[..matched_range.start];
      if NON_ASSIGNMENT_MATCHER.is_match(before) {
        return true;
      }
    }
    let after = &code[matched_range.end..];
    // default delimiters[1] == `\\b(?!\\.)`, we use regex matched `\\b` before
    // needs to test `(?!\\.)` here
    if after.starts_with('.') {
      return true;
    }
    if self.prevent_assignment {
      let stripped_after = after.trim_start();
      if stripped_after.starts_with('=') && !stripped_after[1..].starts_with('=') {
        return true;
      }
    }
    false
  }

  fn fallback_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
    regex: &regress::Regex,
  ) -> bool {
    let mut changed = false;
    for captures in regex.find_iter(code) {
      // We expect the regex we used will always have one `Captures`.
      let Some(Some(matched)) = captures.captures.first() else {
        break;
      };
      if self.prevent_assignment && NON_ASSIGNMENT_MATCHER.is_match(&code[0..matched.start]) {
        continue;
      }
      let Some(replacement) = self.values.get(&code[matched.clone()]) else {
        break;
      };
      changed = true;
      magic_string.update(matched.start, matched.end, replacement);
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
