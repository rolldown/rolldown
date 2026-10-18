//! Test-harness plugin that wraps each module a snapshot hides in a `//#region` pair.
//!
//! `tweak_snapshot` hides the rolldown runtime and the oxc runtime helpers by matching
//! `//#region <id> … //#endregion`. The default `attachDebugInfo: 'simple'` writes only a
//! `// <id>` line where a module starts, and the final output does not show where it ends: the
//! runtime is often followed by the chunk's `export { … }` or `require` lines, not by another
//! module. Before minification the end is still known: the line count of the code that the
//! bundler recorded for the module (`RenderedModule::code`) gives it, even when a template literal
//! in the module holds a blank line. `renderChunk` runs right before the internal minify pass, so
//! this plugin writes the pair there and restores it in `generateBundle`, after every other plugin.
//! Both hooks change only real comment tokens, so the same text inside a string or template
//! literal stays.
//!
//! The pair is disguised as legal comments (`//!#region`). The dce-only pass removes an ordinary
//! comment together with the statement that follows it, and a lost `//#endregion` makes the
//! snapshot hide user code. `full`-mode `//#region` pairs get the same disguise. The end marker
//! replaces the blank line that the chunk joiner puts after each module, so no line moves and the
//! existing sourcemap stays valid.
//!
//! The pairs are a harness artifact: production builds do not have them, but the `dist` files a
//! `_test.mjs` reads do. A fixture's own `generateBundle` hook runs before the restore and
//! observes the disguised `//!#region` form.

use std::borrow::Cow;
use std::sync::Arc;

use oxc::allocator::Allocator;
use oxc::ast::CommentKind;
use oxc::parser::{ParseOptions, Parser};
use oxc::span::SourceType;
use rolldown::plugin::{
  HookGenerateBundleArgs, HookNoopReturn, HookRenderChunkArgs, HookRenderChunkOutput,
  HookRenderChunkReturn, HookTransformOutputMap, HookUsage, Plugin, PluginContext,
};
use rolldown_common::{MinifyOptions, Output, OutputFormat};

#[derive(Debug)]
pub struct HiddenModuleMarkersPlugin {
  /// `TestMeta::hidden_runtime_module`: when `false`, the snapshot shows the runtime as built.
  pub hide_runtime_module: bool,
}

impl Plugin for HiddenModuleMarkersPlugin {
  fn name(&self) -> Cow<'static, str> {
    "rolldown-testing:hidden-module-markers".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk | HookUsage::GenerateBundle
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    // Full minification strips all normal comments on purpose, and a legal-comment disguise
    // would wrongly keep markers alive there.
    if matches!(args.options.minify, MinifyOptions::Enabled(_)) {
      return Ok(None);
    }
    // The recorded code of a module starts with its `// <id>` line.
    let hidden_module_lines: Vec<(String, usize)> = args
      .chunk
      .modules
      .values
      .iter()
      .filter_map(|module| {
        let code = module.code()?;
        let id = code.lines().next()?.strip_prefix("// ")?;
        is_hidden_module(id, self.hide_runtime_module)
          .then(|| (id.to_string(), code.lines().count()))
      })
      .collect();
    let source_type = chunk_source_type(args.options.format);
    let Some(code) = protect_hidden_module_markers(&args.code, source_type, &hidden_module_lines)
    else {
      return Ok(None);
    };
    // Only whole comment lines and blank lines change, so the existing sourcemap stays valid —
    // `Null` states that on purpose (an omitted map would raise SOURCEMAP_BROKEN).
    Ok(Some(HookRenderChunkOutput { code, map: HookTransformOutputMap::Null }))
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    // Same gate as `render_chunk`: a fully minified build carries no disguised markers, and a
    // fixture's own `//!#region`-shaped content must not be rewritten there.
    if matches!(args.options.minify, MinifyOptions::Enabled(_)) {
      return Ok(());
    }
    let source_type = chunk_source_type(args.options.format);
    for output in args.bundle.iter_mut() {
      if let Output::Chunk(chunk) = output {
        if chunk.code.contains("//!#") {
          Arc::make_mut(chunk).code = restore_region_markers(&chunk.code, source_type);
        }
      }
    }
    Ok(())
  }
}

/// Wraps each hidden module of `source` in a disguised `//!#region` pair, and disguises the
/// `//#region` pairs that `full` mode wrote. `hidden_module_lines` holds the id and the line count
/// (marker line included) of each hidden module. Returns `None` when nothing changes.
fn protect_hidden_module_markers(
  source: &str,
  source_type: SourceType,
  hidden_module_lines: &[(String, usize)],
) -> Option<String> {
  let mut out = String::with_capacity(source.len() + 64);
  let mut changed = false;
  let line_count_of = |id: &str| {
    hidden_module_lines.iter().find(|(hidden_id, _)| hidden_id == id).map(|(_, count)| *count)
  };
  // Indentation and end line of the hidden module whose end marker is not written yet.
  let mut open: Option<(&str, usize)> = None;
  // Parsed only when a line looks like a marker, because most chunks have none.
  let mut comment_starts: Option<Vec<u32>> = None;
  let mut is_comment = |offset: usize| {
    comment_starts
      .get_or_insert_with(|| {
        line_comment_starts(source, source_type, |text| {
          text.strip_prefix(' ').is_some_and(|id| line_count_of(id.trim_end()).is_some())
            || is_region_marker(text, "#")
        })
      })
      .binary_search(&u32::try_from(offset).unwrap())
      .is_ok()
  };
  let mut line_start = 0;
  for (line_index, line) in source.split_inclusive('\n').enumerate() {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let trimmed_start = line_start + indent.len();
    line_start += line.len();
    if let Some((module_indent, end_line)) = open
      && line_index == end_line
    {
      open = None;
      out.push_str(module_indent);
      out.push_str("//!#endregion");
      if trimmed.is_empty() {
        // The joiner's blank line after the module becomes its end marker, so no line moves.
        if line.ends_with('\n') {
          out.push('\n');
        }
        continue;
      }
      out.push('\n');
    }
    if let Some(id) = trimmed.strip_prefix("// ")
      && let Some(line_count) = line_count_of(id.trim_end())
      && is_comment(trimmed_start)
    {
      out.push_str(indent);
      out.push_str("//!#region ");
      out.push_str(id);
      open = Some((indent, line_index + line_count));
      changed = true;
      continue;
    }
    if let Some(text) = trimmed.strip_prefix("//")
      && is_region_marker(text, "#")
      && is_comment(trimmed_start)
    {
      out.push_str(indent);
      out.push_str("//!");
      out.push_str(text);
      changed = true;
      continue;
    }
    out.push_str(line);
  }
  if let Some((module_indent, _)) = open {
    // The hidden module is the last source of the chunk, so no blank line follows it.
    if !out.ends_with('\n') {
      out.push('\n');
    }
    out.push_str(module_indent);
    out.push_str("//!#endregion");
    if source.ends_with('\n') {
      out.push('\n');
    }
  }
  changed.then_some(out)
}

/// Turns the disguised `//!#region` and `//!#endregion` comments back into `//#` form.
fn restore_region_markers(source: &str, source_type: SourceType) -> String {
  let comment_starts =
    line_comment_starts(source, source_type, |text| is_region_marker(text, "!#"));
  let mut out = String::with_capacity(source.len());
  let mut line_start = 0;
  for line in source.split_inclusive('\n') {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let trimmed_start = line_start + indent.len();
    line_start += line.len();
    if trimmed.starts_with("//!#")
      && comment_starts.binary_search(&u32::try_from(trimmed_start).unwrap()).is_ok()
    {
      out.push_str(indent);
      out.push_str("//");
      out.push_str(&trimmed["//!".len()..]);
    } else {
      out.push_str(line);
    }
  }
  out
}

/// Sorted start offsets of the line comments whose text after `//` passes `keep`. Marker-like
/// text inside a string or template literal is not a comment token, so it never matches.
fn line_comment_starts(
  source: &str,
  source_type: SourceType,
  keep: impl Fn(&str) -> bool,
) -> Vec<u32> {
  let allocator = Allocator::default();
  let ret = Parser::new(&allocator, source, source_type.with_jsx(true))
    .with_options(ParseOptions { allow_return_outside_function: true, ..Default::default() })
    .parse();
  ret
    .program
    .comments
    .iter()
    .filter(|comment| {
      comment.kind == CommentKind::Line && keep(comment.content_span().source_text(source))
    })
    .map(|comment| comment.span.start)
    .collect()
}

/// Whether a line comment's text after `//` starts with `{prefix}region` or `{prefix}endregion`.
fn is_region_marker(text: &str, prefix: &str) -> bool {
  text
    .strip_prefix(prefix)
    .is_some_and(|rest| rest.starts_with("region") || rest.starts_with("endregion"))
}

/// The module ids whose output `tweak_snapshot` replaces with a `// HIDDEN [...]` line.
fn is_hidden_module(id: &str, hide_runtime_module: bool) -> bool {
  (hide_runtime_module && id == r"\0rolldown/runtime.js")
    || id == "rolldown:hmr"
    || id.starts_with(r"\0@oxc-project+runtime@")
}

/// The source type that the harness syntax check uses for a chunk of this format.
fn chunk_source_type(format: OutputFormat) -> SourceType {
  match format {
    OutputFormat::Cjs => SourceType::cjs(),
    OutputFormat::Esm | OutputFormat::Iife | OutputFormat::Umd => SourceType::mjs(),
  }
}

#[cfg(test)]
mod tests {
  use oxc::span::SourceType;

  use super::{is_hidden_module, protect_hidden_module_markers, restore_region_markers};

  fn hidden_lines(lines: &[(&str, usize)]) -> Vec<(String, usize)> {
    lines.iter().map(|(id, count)| ((*id).to_string(), *count)).collect()
  }

  #[test]
  fn hidden_modules_get_a_region_pair_without_moving_lines() {
    let source = "// \\0rolldown/runtime.js\nvar __a = 1;\n\n// a.js\nconst s = \"// \\0rolldown/runtime.js\";\nexport { __a };\n\t// \\0@oxc-project+runtime@0.1.0/helpers/x.js\n\tvar x = 1;\n";
    let lines = hidden_lines(&[
      (r"\0rolldown/runtime.js", 2),
      (r"\0@oxc-project+runtime@0.1.0/helpers/x.js", 2),
    ]);
    let protected = protect_hidden_module_markers(source, SourceType::mjs(), &lines).unwrap();
    assert_eq!(
      protected,
      "//!#region \\0rolldown/runtime.js\nvar __a = 1;\n//!#endregion\n// a.js\nconst s = \"// \\0rolldown/runtime.js\";\nexport { __a };\n\t//!#region \\0@oxc-project+runtime@0.1.0/helpers/x.js\n\tvar x = 1;\n\t//!#endregion\n"
    );
    assert_eq!(
      restore_region_markers(&protected, SourceType::mjs()),
      "//#region \\0rolldown/runtime.js\nvar __a = 1;\n//#endregion\n// a.js\nconst s = \"// \\0rolldown/runtime.js\";\nexport { __a };\n\t//#region \\0@oxc-project+runtime@0.1.0/helpers/x.js\n\tvar x = 1;\n\t//#endregion\n"
    );
  }

  #[test]
  fn shown_runtime_is_left_as_built() {
    // `render_chunk` leaves a shown runtime out of the hidden-module list.
    assert!(!is_hidden_module(r"\0rolldown/runtime.js", false));
    assert_eq!(
      protect_hidden_module_markers(
        "// \\0rolldown/runtime.js\nvar __a = 1;\n",
        SourceType::mjs(),
        &[]
      ),
      None
    );
  }

  #[test]
  fn hidden_module_ends_at_its_recorded_line_count() {
    let source = "// \\0rolldown/runtime.js\nvar t = `a\n\nb`;\n\nexport { t };\n";
    let lines = hidden_lines(&[(r"\0rolldown/runtime.js", 4)]);
    assert_eq!(
      protect_hidden_module_markers(source, SourceType::mjs(), &lines).unwrap(),
      "//!#region \\0rolldown/runtime.js\nvar t = `a\n\nb`;\n//!#endregion\nexport { t };\n"
    );
  }

  #[test]
  fn marker_text_inside_a_template_literal_is_left_as_built() {
    let source = "const a = `x\n// rolldown:hmr\n\ny`;\nconst b = `x\n// \\0rolldown/runtime.js\ny`;\n\nconst c = `x\n//#region literal\n//!#region literal\ny`;\nexport { a, b, c };\n";
    let lines = hidden_lines(&[("rolldown:hmr", 2), (r"\0rolldown/runtime.js", 2)]);
    assert_eq!(protect_hidden_module_markers(source, SourceType::mjs(), &lines), None);
    assert_eq!(restore_region_markers(source, SourceType::mjs()), source);
  }

  #[test]
  fn full_mode_pairs_are_disguised_reversibly() {
    let source = "//#region a.js\n\tvar a = 1;\n\t//#endregion\nconst s = \"//#region not a marker\";\n//# sourceMappingURL=x.map\n";
    let protected = protect_hidden_module_markers(source, SourceType::mjs(), &[]).unwrap();
    assert_eq!(
      protected,
      "//!#region a.js\n\tvar a = 1;\n\t//!#endregion\nconst s = \"//#region not a marker\";\n//# sourceMappingURL=x.map\n"
    );
    assert_eq!(restore_region_markers(&protected, SourceType::mjs()), source);
  }
}
