use std::borrow::Cow;
use std::ops::Range;
use std::{cmp::Reverse, sync::Arc};

use anyhow::Result;
use rolldown_plugin::{
  HookRenderChunkOutput, HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin,
};
use rolldown_utils::js_regex::HybridRegex;
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, SourceMapOptions};

use crate::utils::expand_typeof_replacements;

#[derive(Debug, Default)]
pub struct ReplaceOptions {
  pub values: FxHashMap</* Target */ String, /* Replacement */ String>,
  /// Default to `("\\b", "\\b(?!\\.)")`. To prevent `typeof window.document` from being replaced by config item `typeof window` => `"object"`.
  pub delimiters: Option<(String, String)>,
  pub prevent_assignment: bool,
  pub object_guards: bool,
  pub sourcemap: bool,
}

#[derive(Debug)]
pub struct ReplacePlugin {
  matcher: HybridRegex,
  prevent_assignment: bool,
  values: FxHashMap</* Target */ String, /* Replacement */ String>,
  sourcemap: bool,
}

// Checks if the given string ends with a variable declaration keyword (const, let, var)
// followed by whitespace, which would indicate the start of a variable declaration.
fn is_variable_declaration_prefix(s: &str) -> bool {
  // First check if there's any whitespace at the end
  if !s.ends_with(|c: char| c.is_whitespace()) {
    return false;
  }

  // Trim the trailing whitespace
  let s = s.trim_end();

  // Inspect the preceding character without mixing byte offsets and character indices.
  s.strip_suffix("const")
    .or_else(|| s.strip_suffix("let"))
    .or_else(|| s.strip_suffix("var"))
    .is_some_and(|prefix| prefix.chars().next_back().is_none_or(|c| !c.is_alphanumeric()))
}

impl ReplacePlugin {
  pub fn new(values: FxHashMap<String, String>) -> Result<Self> {
    Self::with_options(ReplaceOptions { values, ..Default::default() })
  }

  pub fn with_options(options: ReplaceOptions) -> Result<Self> {
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
      HybridRegex::new_ecma(&pattern)?
    } else {
      HybridRegex::from_optimized(
        regex::Regex::new(&format!("\\b({joined_keys})\\b"))
          .expect("to be a valid regex because we escape the keys"),
      )
    };
    Ok(Self {
      matcher,
      prevent_assignment: options.prevent_assignment,
      values,
      sourcemap: options.sourcemap,
    })
  }

  fn try_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> bool {
    if let Some(regex) = self.matcher.as_optimized() {
      self.optimized_replace(code, magic_string, regex)
    } else {
      self.fallback_replace(code, magic_string)
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
      #[expect(clippy::cast_possible_truncation)]
      magic_string
        .update(matched.start() as u32, matched.end() as u32, replacement)
        .expect("update should not fail in replace plugin");
    }

    changed
  }

  fn look_around_assert(&self, code: &str, matched_range: Range<usize>) -> bool {
    if self.prevent_assignment {
      let before = &code[..matched_range.start];
      if is_variable_declaration_prefix(before) {
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
  ) -> bool {
    let mut changed = false;
    let matches =
      self.matcher.find_from(code, 0).expect("fallback regex should use the ECMAScript engine");
    for captures in matches {
      // We expect the regex we used will always have one `Captures`.
      let Some(matched) = captures.group(1) else {
        break;
      };
      if self.prevent_assignment && is_variable_declaration_prefix(&code[0..matched.start]) {
        continue;
      }
      let Some(replacement) = self.values.get(&code[matched.clone()]) else {
        break;
      };
      changed = true;
      #[expect(clippy::cast_possible_truncation)]
      magic_string
        .update(matched.start as u32, matched.end as u32, replacement)
        .expect("update should not fail in replace plugin");
    }
    changed
  }
}

impl Plugin for ReplacePlugin {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    Cow::Borrowed("builtin:replace")
  }

  async fn transform(
    &self,
    _ctx: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    let mut magic_string = MagicString::new(args.code);
    if self.try_replace(args.code, &mut magic_string) {
      return Ok(Some(HookTransformOutput {
        code: Some(magic_string.to_string()),
        map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
          magic_string.source_map(SourceMapOptions {
            hires: string_wizard::Hires::True,
            include_content: false,
            source: Arc::from(args.id),
          })
        }),
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
    let code = args.code.as_str();
    let mut magic_string = MagicString::new(code);
    if self.try_replace(code, &mut magic_string) {
      return Ok(Some(HookRenderChunkOutput {
        code: magic_string.to_string(),
        map: HookTransformOutputMap::from_if_enabled(self.sourcemap, || {
          magic_string.source_map(SourceMapOptions {
            hires: string_wizard::Hires::True,
            include_content: false,
            source: Arc::from(args.chunk.filename.as_str()),
          })
        }),
      }));
    }
    Ok(None)
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform | HookUsage::RenderChunk
  }
}

#[cfg(test)]
mod tests {
  use super::{ReplaceOptions, ReplacePlugin};
  use string_wizard::MagicString;

  #[test]
  fn prevent_assignment_after_unicode() {
    for delimiters in [None, Some((r"\b".to_string(), r"\b(?!\.)".to_string()))] {
      let plugin = ReplacePlugin::with_options(ReplaceOptions {
        values: [("FOO".to_string(), "123".to_string())].into_iter().collect(),
        delimiters,
        prevent_assignment: true,
        ..Default::default()
      })
      .unwrap();

      for prefix in ["", "/*e*/ ", "/*é*/ ", "/*😀*/ "] {
        for declaration in ["let FOO;", "var FOO;", "for (const FOO of []) {}"] {
          let code = format!("{prefix}{declaration} console.log(FOO);");
          let mut magic_string = MagicString::new(&code);
          assert!(plugin.try_replace(&code, &mut magic_string));
          assert_eq!(
            magic_string.to_string(),
            format!("{prefix}{declaration} console.log(123);"),
            "input: {code}"
          );
        }

        // A keyword suffix in a longer word does not begin a declaration.
        for word in ["outlet", "myvar", "myconst", "élet"] {
          let code = format!("{prefix}{word} FOO");
          let mut magic_string = MagicString::new(&code);
          assert!(plugin.try_replace(&code, &mut magic_string));
          assert_eq!(magic_string.to_string(), format!("{prefix}{word} 123"));
        }
      }
    }
  }
}
