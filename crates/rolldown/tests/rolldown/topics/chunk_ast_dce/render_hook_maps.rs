use std::{
  borrow::Cow,
  process::Command,
  sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
  },
};

use rolldown::{BundleOutput, Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  ManglePropertiesPattern, ManglePropertiesPatterns, Output, OutputChunk, RawCompressOptions,
  RawManglePropertiesOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};
use rolldown_error::EventKind;
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookTransformArgs,
  HookTransformOutput, HookTransformOutputMap, HookTransformReturn, HookUsage, Plugin,
  PluginContext, SharedTransformPluginContext,
};
use string_wizard::{MagicString, SourceMapOptions};

const MAIN: &str = include_str!("path_coverage/cross/main.js");
const DEP: &str = include_str!("path_coverage/cross/dep.js");
const PREFIX: &str = "// render hook shifted this chunk\n// retain the producer transform chain\n";
const SUFFIX: &str = "\nglobalThis.__hookResult = 6 * 7;\n";
const PROPERTIES: &str = "\nglobalThis.__hookObject = { _hook: 6 * 7, _reserved: 'kept' };\nglobalThis.__hookResult = globalThis.__hookObject._hook;\n";

type Observations = Arc<Mutex<Vec<(&'static str, String)>>>;

#[derive(Debug)]
struct ShiftProducers(Arc<AtomicUsize>);

impl Plugin for ShiftProducers {
  fn name(&self) -> Cow<'static, str> {
    "render-map-shift-producers".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::Transform
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if !args.id.ends_with(".js") || args.id.starts_with('\0') {
      return Ok(None);
    }
    self.0.fetch_add(1, Ordering::Relaxed);
    let mut code = MagicString::new(args.code.as_str());
    code.prepend("// producer transform\n// producer transform\n");
    Ok(Some(HookTransformOutput {
      code: Some(code.to_string()),
      map: code
        .source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: Arc::from(args.id),
        })
        .into(),
      ..Default::default()
    }))
  }
}

#[derive(Clone, Copy, Debug)]
enum MapKind {
  Omitted,
  Empty,
  Identity,
}

#[derive(Clone, Copy, Debug)]
enum Action {
  None,
  Identity(MapKind),
  Edit { prefix: &'static str, suffix: &'static str },
}

#[derive(Debug)]
struct RenderHook {
  label: &'static str,
  action: Action,
  observations: Observations,
}

impl Plugin for RenderHook {
  fn name(&self) -> Cow<'static, str> {
    self.label.into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    self.observations.lock().unwrap().push((self.label, args.code.to_string()));
    if matches!(self.action, Action::None) {
      return Ok(None);
    }
    let mut code = MagicString::new(args.code.as_str());
    if let Action::Edit { prefix, suffix } = self.action {
      code.prepend(prefix);
      code.append(suffix);
    }
    let map = match self.action {
      Action::Identity(MapKind::Omitted) => HookTransformOutputMap::Omitted,
      Action::Identity(MapKind::Empty) => rolldown_sourcemap::empty_sourcemap().into(),
      _ => code
        .source_map(SourceMapOptions {
          hires: string_wizard::Hires::True,
          include_content: true,
          source: Arc::from(args.chunk.filename.as_str()),
        })
        .into(),
    };
    Ok(Some(HookRenderChunkOutput { code: code.to_string(), map }))
  }
}

fn options(full: bool) -> BundlerOptions {
  BundlerOptions {
    cwd: Some(
      concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/path_coverage")
        .into(),
    ),
    input: Some(vec![InputItem { name: Some("main".into()), import: "./cross/main.js".into() }]),
    entry_filenames: Some("main.mjs".to_string().into()),
    sourcemap: Some(SourceMapType::File),
    minify: full.then(|| RawMinifyOptions::Bool(true)),
    ..Default::default()
  }
}

fn chunk(output: &BundleOutput) -> &OutputChunk {
  let mut chunks = output
    .assets
    .iter()
    .filter_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk.as_ref()) } else { None });
  let chunk = chunks.next().unwrap();
  assert!(chunks.next().is_none());
  chunk
}

fn final_map(chunk: &OutputChunk) -> oxc_sourcemap::SourceMap<'static> {
  let json = chunk.map.as_ref().unwrap().to_json_string();
  let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap().into_owned();
  super::map_assertions::assert_valid_map(&chunk.code, &map);
  map
}

fn assert_original_anchors(chunk: &OutputChunk, map: &oxc_sourcemap::SourceMap<'_>) {
  // Side-effectful calls survive both DCE-only and full minification. Their literal starts
  // must trace through both the module transform and renderChunk back to real source text.
  for (marker, source, content) in
    [("cross-main-anchor", "cross/main.js", MAIN), ("cross-dep-anchor", "cross/dep.js", DEP)]
  {
    let generated = chunk.code.find(marker).unwrap() - 1;
    let original = content.find(marker).unwrap() - 1;
    super::map_assertions::assert_mapping_at(
      &chunk.code,
      map,
      generated,
      source,
      content,
      original,
    );
  }
}

fn execute(chunk: &OutputChunk, checks: &str) {
  let script = format!(
    "import assert from 'node:assert/strict';\nglobalThis.__pathEvents = [];\nconst entry = await import('data:text/javascript;base64,' + Buffer.from({}).toString('base64'));\nassert.equal(entry.result.value, 'cross-inlined-value');\nassert.deepEqual(globalThis.__pathEvents, ['cross-dep-anchor', 'cross-main-anchor']);\n{checks}",
    serde_json::to_string(&chunk.code).unwrap(),
  );
  let result =
    Command::new("node").args(["--input-type=module", "--eval", &script]).output().unwrap();
  assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

#[tokio::test(flavor = "multi_thread")]
async fn identity_code_return_distinguishes_omitted_empty_and_identity_maps() {
  for full in [false, true] {
    for kind in [MapKind::Omitted, MapKind::Empty, MapKind::Identity] {
      let transformed = Arc::new(AtomicUsize::new(0));
      let observations = Observations::default();
      let output = Bundler::with_plugins(
        options(full),
        vec![
          Plugin::new_shared(ShiftProducers(Arc::clone(&transformed))),
          Plugin::new_shared(RenderHook {
            label: "identity-render-map",
            action: Action::Identity(kind),
            observations: Arc::clone(&observations),
          }),
        ],
      )
      .unwrap()
      .generate()
      .await
      .unwrap();
      assert_eq!(transformed.load(Ordering::Relaxed), 3);
      let observations = observations.lock().unwrap();
      assert_eq!(observations.len(), 1);
      let chunk = chunk(&output);
      if full {
        assert_ne!(chunk.code, observations[0].1, "hooks must precede optimization");
      }
      let map = final_map(chunk);
      match kind {
        MapKind::Empty => {
          for marker in ["cross-main-anchor", "cross-dep-anchor", "cross-inlined-value"] {
            super::map_assertions::assert_unmapped(&chunk.code, &map, marker);
          }
          assert!(map.get_source_view_tokens().all(|token| token.get_source_id().is_none()));
        }
        MapKind::Omitted | MapKind::Identity => assert_original_anchors(chunk, &map),
      }
      let broken: Vec<_> = output
        .warnings
        .iter()
        .filter(|warning| matches!(warning.kind(), EventKind::SourcemapBroken))
        .collect();
      assert_eq!(broken.len(), usize::from(matches!(kind, MapKind::Omitted)));
      if let Some(warning) = broken.first() {
        assert!(warning.to_string().contains("identity-render-map"));
      }
      execute(
        chunk,
        "assert.equal(entry.result.sum, 42);\nassert.equal(globalThis.__hookResult, undefined);",
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn none_then_some_then_none_preserves_order_and_composed_map() {
  for full in [false, true] {
    let transformed = Arc::new(AtomicUsize::new(0));
    let observations = Observations::default();
    let output = Bundler::with_plugins(
      options(full),
      vec![
        Plugin::new_shared(ShiftProducers(Arc::clone(&transformed))),
        Plugin::new_shared(RenderHook {
          label: "before-replacement",
          action: Action::None,
          observations: Arc::clone(&observations),
        }),
        Plugin::new_shared(RenderHook {
          label: "mapped-replacement",
          action: Action::Edit { prefix: PREFIX, suffix: SUFFIX },
          observations: Arc::clone(&observations),
        }),
        Plugin::new_shared(RenderHook {
          label: "after-replacement",
          action: Action::None,
          observations: Arc::clone(&observations),
        }),
      ],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    assert_eq!(transformed.load(Ordering::Relaxed), 3);
    let observations = observations.lock().unwrap();
    assert_eq!(
      observations.iter().map(|(label, _)| *label).collect::<Vec<_>>(),
      ["before-replacement", "mapped-replacement", "after-replacement",]
    );
    assert_eq!(observations[0].1, observations[1].1);
    assert_eq!(observations[2].1, format!("{PREFIX}{}{SUFFIX}", observations[1].1));
    let chunk = chunk(&output);
    assert!(!chunk.code.contains("6 * 7"));
    let map = final_map(chunk);
    assert_original_anchors(chunk, &map);
    super::map_assertions::assert_unmapped(&chunk.code, &map, "__hookResult");
    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
    execute(
      chunk,
      "assert.equal(entry.result.sum, 42);\nassert.equal(globalThis.__hookResult, 42);",
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn replacement_text_returns_property_cache_with_seeded_reservations() {
  let pattern = "^(sum|value|_hook|_reserved)$";
  let mut properties = oxc::minifier::ManglePropertiesOptions::from_pattern(pattern).unwrap();
  properties.cache.insert("sum".into(), Some("cachedSum".into())).unwrap();
  for reserved in ["value", "_reserved", "_unused", "a"] {
    properties.cache.insert(reserved.into(), None).unwrap();
  }
  let observations = Observations::default();
  let output = Bundler::with_plugins(
    BundlerOptions {
      minify: Some(RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        compress: Some(RawCompressOptions::default()),
        mangle: None,
        mangle_properties: Some(Box::new(RawManglePropertiesOptions {
          options: properties,
          patterns: ManglePropertiesPatterns {
            include: ManglePropertiesPattern { source: pattern.into(), flags: String::new() },
            exclude: None,
          },
        })),
        remove_whitespace: true,
        ascii_only: false,
      })),
      ..options(true)
    },
    vec![Plugin::new_shared(RenderHook {
      label: "property-text-replacement",
      action: Action::Edit { prefix: "", suffix: PROPERTIES },
      observations: Arc::clone(&observations),
    })],
  )
  .unwrap()
  .generate()
  .await
  .unwrap();
  assert_eq!(observations.lock().unwrap().len(), 1);
  let cache = output.mangle_cache.as_ref().expect("hook text optimization must return its cache");
  assert_eq!(cache.get("sum"), Some(&Some("cachedSum".to_owned())));
  for reserved in ["value", "_reserved", "_unused", "a"] {
    assert_eq!(cache.get(reserved), Some(&None), "lost cache reservation {reserved}");
  }
  let hook_name =
    cache.get("_hook").and_then(Option::as_ref).expect("new hook property needs a cache entry");
  assert_ne!(hook_name, "_hook");
  assert!(!["cachedSum", "value", "_reserved", "_unused", "a"].contains(&hook_name.as_str()));
  let chunk = chunk(&output);
  assert!(!chunk.code.contains("6 * 7"));
  let map = final_map(chunk);
  assert_original_anchors(chunk, &map);
  for marker in ["__hookObject", "__hookResult"] {
    super::map_assertions::assert_unmapped(&chunk.code, &map, marker);
  }
  assert!(output.warnings.is_empty(), "{:?}", output.warnings);
  execute(
    chunk,
    &format!(
      "assert.equal(entry.result.cachedSum, 42);\nassert.equal(entry.result.sum, undefined);\nassert.equal(globalThis.__hookResult, 42);\nassert.deepEqual(globalThis.__hookObject, {{ [{}]: 42, _reserved: 'kept' }});",
      serde_json::to_string(hook_name).unwrap(),
    ),
  );
}
