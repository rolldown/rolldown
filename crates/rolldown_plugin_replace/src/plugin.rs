use std::borrow::Cow;
use std::future::Future;
use std::ops::Range;
use std::pin::Pin;
use std::{cmp::Reverse, sync::Arc};

use anyhow::Result;
use rolldown_plugin::{
  HookRenderChunkOutput, HookTransformOutput, HookTransformOutputMap, HookUsage, Plugin,
};
use rustc_hash::FxHashMap;
use string_wizard::{MagicString, SourceMapOptions};

use crate::utils::expand_typeof_replacements;

/// A replacement computed on the JS side. Called with the module id and the matched target.
pub type ReplacementFn = dyn Fn(
    /* id */ String,
    /* target */ String,
  ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send>>
  + Send
  + Sync;

#[derive(derive_more::Debug, Default)]
pub struct ReplaceOptions {
  pub values: FxHashMap</* Target */ String, /* Replacement */ String>,
  /// Targets whose replacement is computed per module by a callback.
  #[debug(skip)]
  pub value_callbacks: FxHashMap</* Target */ String, Arc<ReplacementFn>>,
  /// Default to `("\\b", "\\b(?!\\.)")`. To prevent `typeof window.document` from being replaced by config item `typeof window` => `"object"`.
  pub delimiters: Option<(String, String)>,
  pub prevent_assignment: bool,
  pub object_guards: bool,
  pub sourcemap: bool,
}

// We don't reuse `HybridRegex` in `rolldown_utils`, since
// only the enum is needed
#[derive(Debug)]
enum HybridRegex {
  Optimize(regex::Regex),
  Ecma(regress::Regex),
}

#[derive(derive_more::Debug)]
pub struct ReplacePlugin {
  matcher: HybridRegex,
  prevent_assignment: bool,
  values: FxHashMap</* Target */ String, /* Replacement */ String>,
  #[debug(skip)]
  value_callbacks: FxHashMap</* Target */ String, Arc<ReplacementFn>>,
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

  // Check for word boundary before the keywords
  (s.ends_with("const")
    && (s.len() == 5 || !s.chars().nth(s.len() - 6).unwrap_or(' ').is_alphanumeric()))
    || (s.ends_with("let")
      && (s.len() == 3 || !s.chars().nth(s.len() - 4).unwrap_or(' ').is_alphanumeric()))
    || (s.ends_with("var")
      && (s.len() == 3 || !s.chars().nth(s.len() - 4).unwrap_or(' ').is_alphanumeric()))
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
    let value_callbacks = options.value_callbacks;
    let mut keys = values.keys().chain(value_callbacks.keys()).collect::<Vec<_>>();
    // Sort by length in descending order so that longer targets are matched first.
    keys.sort_by_key(|key| Reverse(key.len()));

    let lookahead = if options.prevent_assignment { "(?!\\s*=[^=])" } else { "" };

    let joined_keys = keys.iter().map(|key| regex::escape(key)).collect::<Vec<_>>().join("|");
    // https://rustexp.lpil.uk/
    let matcher = if let Some((delimiter_left, delimiter_right)) = options.delimiters {
      let pattern = format!("{delimiter_left}({joined_keys}){delimiter_right}{lookahead}");
      HybridRegex::Ecma(regress::Regex::new(&pattern)?)
    } else {
      HybridRegex::Optimize(
        regex::Regex::new(&format!("\\b({joined_keys})\\b"))
          .expect("to be a valid regex because we escape the keys"),
      )
    };
    Ok(Self {
      matcher,
      prevent_assignment: options.prevent_assignment,
      values,
      value_callbacks,
      sourcemap: options.sourcemap,
    })
  }

  /// Walks every match of the target regex and hands it to `on_match`, which returns `false` to
  /// stop the walk.
  fn scan<'text>(
    &self,
    code: &'text str,
    mut on_match: impl FnMut(/* target */ &'text str, Range<usize>) -> bool,
  ) {
    match self.matcher {
      HybridRegex::Optimize(ref regex) => {
        for captures in regex.captures_iter(code) {
          let Some(matched) = captures.get(1) else {
            break;
          };
          if self.look_around_assert(code, matched.range()) {
            continue;
          }
          if !on_match(matched.as_str(), matched.range()) {
            break;
          }
        }
      }
      HybridRegex::Ecma(ref regex) => {
        for captures in regex.find_iter(code) {
          // We expect the regex we used will always have one `Captures`.
          let Some(Some(matched)) = captures.captures.first() else {
            break;
          };
          if self.prevent_assignment && is_variable_declaration_prefix(&code[0..matched.start]) {
            continue;
          }
          if !on_match(&code[matched.clone()], matched.clone()) {
            break;
          }
        }
      }
    }
  }

  fn try_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> bool {
    let mut changed = false;
    self.scan(code, |target, range| {
      let Some(replacement) = self.values.get(target) else {
        return false;
      };
      changed = true;
      #[expect(clippy::cast_possible_truncation)]
      magic_string
        .update(range.start as u32, range.end as u32, replacement.as_str())
        .expect("update should not fail in replace plugin");
      true
    });
    changed
  }

  /// The slow path. Only used when at least one target has a callback replacement, because it
  /// collects the matches first and then calls into JS for each match of a callback target.
  async fn try_replace_with_callbacks<'text>(
    &'text self,
    id: &str,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> Result<bool> {
    let mut matches = Vec::new();
    self.scan(code, |target, range| {
      if !self.values.contains_key(target) && !self.value_callbacks.contains_key(target) {
        return false;
      }
      matches.push((target, range));
      true
    });

    // A callback only gets the module id, so its result is the same for every match of the same
    // target in the same module. We call it once per target instead of once per match.
    let mut computed = FxHashMap::<&str, String>::default();
    let mut changed = false;
    for (target, range) in matches {
      #[expect(clippy::cast_possible_truncation)]
      let (start, end) = (range.start as u32, range.end as u32);
      if let Some(replacement) = self.values.get(target) {
        magic_string
          .update(start, end, replacement.as_str())
          .expect("update should not fail in replace plugin");
      } else if let Some(callback) = self.value_callbacks.get(target) {
        let replacement = match computed.get(target) {
          Some(replacement) => replacement.clone(),
          None => {
            let replacement = callback(id.to_string(), target.to_string()).await?;
            computed.insert(target, replacement.clone());
            replacement
          }
        };
        magic_string
          .update(start, end, replacement)
          .expect("update should not fail in replace plugin");
      }
      changed = true;
    }
    Ok(changed)
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
    let changed = if self.value_callbacks.is_empty() {
      self.try_replace(args.code, &mut magic_string)
    } else {
      self.try_replace_with_callbacks(args.id, args.code, &mut magic_string).await?
    };
    if changed {
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
    let changed = if self.value_callbacks.is_empty() {
      self.try_replace(code, &mut magic_string)
    } else {
      self
        .try_replace_with_callbacks(&args.chunk.filename, code, &mut magic_string)
        .await?
    };
    if changed {
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
