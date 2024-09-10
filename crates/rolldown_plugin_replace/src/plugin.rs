use std::{cmp::Reverse, collections::HashMap, sync::LazyLock};

use fancy_regex::{Regex, RegexBuilder};
use rolldown_plugin::{HookRenderChunkOutput, HookTransformOutput, Plugin};
use rustc_hash::FxHashMap;
use string_wizard::MagicString;

use crate::utils::expand_typeof_replacements;

#[derive(Debug)]
pub struct ReplaceOptions {
  pub values: HashMap</* Target */ String, /* Replacement */ String>,
  /// Default to `("\\b", "\\b(?!\\.)")`. To prevent `typeof window.document` from being replaced by config item `typeof window` => `"object"`.
  pub delimiters: (String, String),
  pub prevent_assignment: bool,
  pub object_guards: bool,
}

impl Default for ReplaceOptions {
  fn default() -> Self {
    Self {
      values: HashMap::default(),
      delimiters: ("\\b".to_string(), "\\b(?!\\.)".to_string()),
      prevent_assignment: false,
      object_guards: false,
    }
  }
}

#[derive(Debug)]
pub struct ReplacePlugin {
  matcher: Regex,
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

    let joined_keys = keys.iter().map(|key| fancy_regex::escape(key)).collect::<Vec<_>>().join("|");
    let (delimiter_left, delimiter_right) = &options.delimiters;
    // https://rustexp.lpil.uk/
    let pattern = format!("{delimiter_left}({joined_keys}){delimiter_right}{lookahead}");
    Self {
      matcher: RegexBuilder::new(&pattern)
        // Set `backtrack_limit` for `delimiters` and `lookahead` because they contain backtracking pattern `!?` and `*`.
        // Cannot set the number too low or it will be ignored by `fancy_regex::Error::RuntimeError` in `try_replace`.
        // Setting `backtrack_limit` to a large number will cause huge performance regression.
        // See <https://github.com/fancy-regex/fancy-regex/blob/main/PERFORMANCE.md#fancy-regex>.
        .backtrack_limit(1000)
        .build()
        .unwrap_or_else(|_| panic!("Invalid regex {pattern:?}")),
      prevent_assignment: options.prevent_assignment,
      values: values.into_iter().collect(),
    }
  }

  fn try_replace<'text>(
    &'text self,
    code: &'text str,
    magic_string: &mut MagicString<'text>,
  ) -> anyhow::Result<bool> {
    let mut changed = false;
    for captures in self.matcher.captures_iter(code) {
      // We expect the regex we used will always have one `Captures`.

      let captures = match captures {
        Ok(inner) => inner,
        Err(err) => match err {
          fancy_regex::Error::RuntimeError(_) => {
            // Mostly due to backtrack limit exceeded. There's nothing we can do about runtime error.
            // So if we encounter one, we just consider this as a failed match and skip it.
            break;
          }
          _ => return Err(err.into()),
        },
      };
      let Some(matched) = captures.get(1) else {
        break;
      };
      if self.prevent_assignment
        && NON_ASSIGNMENT_MATCHER.is_match(&code[0..matched.start()]).unwrap_or(false)
      {
        continue;
      }
      let Some(replacement) = self.values.get(matched.as_str()) else {
        break;
      };
      changed = true;
      magic_string.update(matched.start(), matched.end(), replacement);
    }

    Ok(changed)
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
    if self.try_replace(args.code, &mut magic_string)? {
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
    if self.try_replace(&args.code, &mut magic_string)? {
      return Ok(Some(HookRenderChunkOutput { code: magic_string.to_string(), map: None }));
    }
    Ok(None)
  }
}
