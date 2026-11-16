//! Lazy-init ordering moves `init_second()` (wrapped ESM) and
//! `var import_first = __toESM(require_first())` (CJS) from `main.js` to the front of
//! `consumer.js`. The moved statements are AST fragments prepended to the recipient's AST.
//! See internal-docs/ast-mutation/implementation.md.
//!
//! Checks the serialized-and-reparsed sourcemap: the moved statements are unmapped, while the
//! recipient (which also carries a plugin transform map), the module after it and the donor keep
//! their own mappings.

use std::{borrow::Cow, path::PathBuf, sync::Arc};

use rolldown::{Bundler, BundlerOptions, InputItem, OutputFormat, SourceMapType};
use rolldown_common::{Output, RawMinifyOptions};
use rolldown_plugin::{
  HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage, Plugin,
  SharedTransformPluginContext,
};
use rolldown_sourcemap::OwnedSourceMap;

fn fixture_dir() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/transferred_imports"))
}

/// Prepends a line to `consumer.js`, so its final map is composed from a plugin map.
#[derive(Debug)]
struct PrependTransform;

impl Plugin for PrependTransform {
  fn name(&self) -> Cow<'static, str> {
    "prepend-transform".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with("consumer.js") {
      return Ok(None);
    }
    let mut magic_string = string_wizard::MagicString::new(args.code.as_str());
    magic_string.prepend("globalThis.__events.push('plugin-banner');\n");
    let map = magic_string.source_map(string_wizard::SourceMapOptions {
      include_content: true,
      source: Arc::from(args.id),
      hires: string_wizard::Hires::Boundary,
    });
    Ok(Some(HookTransformOutput {
      code: Some(magic_string.to_string()),
      map: map.into(),
      ..Default::default()
    }))
  }
}

/// 0-based `(line, column)` of the only occurrence of `needle`.
fn position_of(text: &str, needle: &str) -> (u32, u32) {
  let mut matches = text.match_indices(needle);
  let (offset, _) = matches.next().unwrap_or_else(|| panic!("{needle:?} not found in:\n{text}"));
  assert!(matches.next().is_none(), "{needle:?} is not unique in:\n{text}");
  let before = &text[..offset];
  let line = before.matches('\n').count();
  let column = before.len() - before.rfind('\n').map_or(0, |newline| newline + 1);
  (u32::try_from(line).unwrap(), u32::try_from(column).unwrap())
}

/// 0-based `(line, indent)` of the only line whose trimmed text is `line_text`.
fn line_of(code: &str, line_text: &str) -> (usize, usize) {
  let mut lines = code.lines().enumerate().filter(|(_, line)| line.trim() == line_text);
  let (index, line) = lines.next().unwrap_or_else(|| panic!("no line {line_text:?} in:\n{code}"));
  assert!(lines.next().is_none(), "line {line_text:?} is not unique in:\n{code}");
  (index, line.len() - line.trim_start().len())
}

/// The generated string literal `literal` must map to the same literal in `file`.
fn assert_maps_to(code: &str, map: &OwnedSourceMap, literal: &str, file: &str) {
  let map = map.as_source_map();
  // Minified output prints string literals as template literals.
  let needle = ["\"", "`"]
    .map(|quote| format!("{quote}{literal}{quote}"))
    .into_iter()
    .find(|needle| code.contains(needle.as_str()))
    .unwrap_or_else(|| panic!("{literal:?} not found in:\n{code}"));
  let (line, column) = position_of(code, &needle);
  let token = map
    .lookup_token(&map.generate_lookup_table(), line, column)
    .filter(|token| (token.get_dst_line(), token.get_dst_col()) == (line, column))
    .unwrap_or_else(|| panic!("{needle} at {line}:{column} has no token of its own"));
  let source = token.get_source_id().and_then(|id| map.get_source(id));
  assert!(source.is_some_and(|source| source.ends_with(file)), "{needle} maps to {source:?}");
  let content = std::fs::read_to_string(fixture_dir().join(file)).unwrap();
  assert_eq!(
    (token.get_src_line(), token.get_src_col()),
    position_of(&content, &format!("'{literal}'")),
    "{needle} maps to the wrong position in {file}"
  );
}

#[tokio::test(flavor = "multi_thread")]
async fn transferred_imports_are_unmapped_and_keep_neighbour_mappings() {
  for (format, minify) in [(OutputFormat::Iife, false), (OutputFormat::Esm, true)] {
    let mut bundler = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(fixture_dir()),
        input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
        format: Some(format),
        minify: Some(RawMinifyOptions::Bool(minify)),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![Plugin::new_shared(PrependTransform)],
    )
    .unwrap();
    let output = bundler.generate().await.unwrap();
    let Some(Output::Chunk(chunk)) = output.assets.first() else { panic!("should emit a chunk") };
    let code = chunk.code.as_str();
    // Check the parsed JSON, not the in-memory map.
    let map =
      OwnedSourceMap::from_json_string(&chunk.map.as_ref().unwrap().to_json_string()).unwrap();

    assert_maps_to(code, &map, "consumer-inner-anchor", "consumer.js");
    assert_maps_to(code, &map, "consumer-anchor", "consumer.js");
    assert_maps_to(code, &map, "after-anchor", "after.js");
    assert_maps_to(code, &map, "main-anchor", "main.js");
    if minify {
      continue;
    }

    let recipient = line_of(code, "// consumer.js");
    let moved = [
      line_of(code, "init_second();"),
      line_of(code, "var import_first = /* @__PURE__ */ __toESM(require_first());"),
    ];
    let recipient_own = line_of(code, "function consumerHelper(input) {");
    // Prepended to the recipient in `insert_map` order, with the recipient's indentation.
    assert!(recipient.0 < moved[0].0 && moved[0].0 < moved[1].0 && moved[1].0 < recipient_own.0);
    assert!(moved.iter().all(|line| line.1 == recipient_own.1), "{code}");
    // Unmapped: attributed neither to the recipient's source nor to the donor's offsets.
    let moved_lines = moved.map(|(line, _)| u32::try_from(line).unwrap());
    let stray: Vec<_> =
      map.get_tokens().filter(|token| moved_lines.contains(&token.get_dst_line())).collect();
    assert!(stray.is_empty(), "{format:?}: transferred statements have tokens {stray:?}");
  }
}
