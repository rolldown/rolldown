use std::{borrow::Cow, path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  AddonOutputOption, CommentsOptions, EmittedAsset, ManglePropertiesPattern,
  ManglePropertiesPatterns, Output, OutputFormat, RawCompressOptions, RawMangleOptions,
  RawManglePropertiesOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};
use rolldown_plugin::{
  HookRenderChunkArgs, HookRenderChunkOutput, HookRenderChunkReturn, HookUsage, Plugin,
  PluginContext,
};
use rolldown_std_utils::normalize_path_buf_to_slash;

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/path_coverage");

#[derive(Debug)]
struct RenderChunkFallback;

impl Plugin for RenderChunkFallback {
  fn name(&self) -> Cow<'static, str> {
    "path-coverage-render-chunk".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    Ok(Some(HookRenderChunkOutput {
      code: format!("{}\nglobalThis.__pathHook = 17;", args.code),
      map: rolldown_plugin::HookTransformOutputMap::default(),
    }))
  }
}

fn minify(mode: &str) -> RawMinifyOptions {
  if mode == "full" {
    return RawMinifyOptions::Bool(true);
  }
  RawMinifyOptions::Object(RawMinifyOptionsDetailed {
    compress: (mode == "compress_only").then(RawCompressOptions::default),
    mangle: (mode == "mangle_only").then(RawMangleOptions::default),
    mangle_properties: None,
    remove_whitespace: false,
    ascii_only: false,
  })
}

fn assert_anchors(chunks: &[&rolldown_common::OutputChunk], graph: &str) -> usize {
  let sources: &[(&str, &str, &str)] = match graph {
    "cross_module" => &[
      ("cross/main.js", include_str!("path_coverage/cross/main.js"), "cross-main-anchor"),
      ("cross/dep.js", include_str!("path_coverage/cross/dep.js"), "cross-dep-anchor"),
    ],
    "private_fields" => &[
      ("private/main.js", include_str!("path_coverage/private/main.js"), "private-main-anchor"),
      ("private/first.js", include_str!("path_coverage/private/first.js"), "private-first-anchor"),
      (
        "private/second.js",
        include_str!("path_coverage/private/second.js"),
        "private-second-anchor",
      ),
    ],
    "splitting" => &[
      ("splitting/main.js", include_str!("path_coverage/splitting/main.js"), "split-main-anchor"),
      ("splitting/lazy.js", include_str!("path_coverage/splitting/lazy.js"), "split-lazy-anchor"),
      (
        "splitting/shared.js",
        include_str!("path_coverage/splitting/shared.js"),
        "split-shared-anchor",
      ),
    ],
    _ => unreachable!(),
  };
  for &(source, content, marker) in sources {
    let owners: Vec<_> = chunks.iter().filter(|chunk| chunk.code.contains(marker)).collect();
    assert_eq!(owners.len(), 1, "anchor must have one final owner: {marker}");
    super::map_assertions::assert_mapping(
      &owners[0].code,
      owners[0].map.as_ref().unwrap(),
      marker,
      source,
      content,
      &format!("'{marker}'"),
    );
  }
  sources.len()
}

// Path evidence is asserted by the external runner using ROLLDOWN_CHUNK_AST_COVERAGE, never by
// changing process-global environment variables while Tokio/Rayon workers may be using them.
#[tokio::test(flavor = "multi_thread")]
async fn execution_and_final_maps() {
  let pipeline = "chunk-ast";
  let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let mut captures = Vec::new();
  let mut builds = 0;
  let mut map_anchors = 0;
  let mut chunk_count = 0;
  for scenario in [
    "cross_module",
    "private_fields",
    "splitting",
    "cjs",
    "iife",
    "render_chunk",
    "intro",
    "comment_banner",
    "code_banner",
  ] {
    let (graph, entry, format) = match scenario {
      "private_fields" => ("private_fields", "./private/main.js", OutputFormat::Esm),
      "splitting" => ("splitting", "./splitting/main.js", OutputFormat::Esm),
      "cjs" => ("cross_module", "./cross/main.js", OutputFormat::Cjs),
      "iife" => ("cross_module", "./cross/main.js", OutputFormat::Iife),
      _ => ("cross_module", "./cross/main.js", OutputFormat::Esm),
    };
    for mode in ["compress_only", "mangle_only", "full"] {
      let out_dir =
        workspace.join("tmp/adoption-audit/path-coverage").join(pipeline).join(scenario).join(mode);
      std::fs::create_dir_all(&out_dir).unwrap();
      let extension = if matches!(format, OutputFormat::Esm) { "mjs" } else { "cjs" };
      let mut options = BundlerOptions {
        cwd: Some(FIXTURE.into()),
        dir: Some(normalize_path_buf_to_slash(out_dir.clone())),
        input: Some(vec![InputItem { name: Some("main".into()), import: entry.into() }]),
        format: Some(format),
        name: matches!(format, OutputFormat::Iife).then(|| "PathCoverage".into()),
        entry_filenames: Some(format!("[name].{extension}").into()),
        chunk_filenames: Some(format!("[name]-[hash].{extension}").into()),
        minify: Some(minify(mode)),
        sourcemap: Some(SourceMapType::File),
        comments: Some(CommentsOptions { legal: true, annotation: false, jsdoc: false }),
        ..Default::default()
      };
      match scenario {
        "intro" => {
          options.intro =
            Some(AddonOutputOption::String(Some("globalThis.__pathIntro = 19;".into())));
        }
        "comment_banner" => {
          options.banner =
            Some(AddonOutputOption::String(Some("/*! path-comment-banner */".into())));
        }
        "code_banner" => {
          options.banner =
            Some(AddonOutputOption::String(Some("globalThis.__pathBanner = 23;".into())));
        }
        _ => {}
      }
      let plugins = if scenario == "render_chunk" {
        vec![Plugin::new_shared(RenderChunkFallback)]
      } else {
        vec![]
      };
      let output = Bundler::with_plugins(options, plugins).unwrap().generate().await;
      let output = output.unwrap();
      let chunks: Vec<_> = output
        .assets
        .iter()
        .filter_map(|asset| match asset {
          Output::Chunk(chunk) => Some(chunk.as_ref()),
          Output::Asset(_) => None,
        })
        .collect();
      assert_eq!(chunks.len(), if scenario == "splitting" { 3 } else { 1 }, "{scenario}/{mode}");
      let entry_chunk = chunks.iter().find(|chunk| chunk.is_entry).unwrap();
      if scenario == "splitting" {
        assert_eq!(chunks.iter().filter(|chunk| chunk.is_dynamic_entry).count(), 1);
        assert_eq!(entry_chunk.dynamic_imports.len(), 1);
        assert!(!entry_chunk.imports.is_empty(), "entry must share a chunk with the lazy entry");
      }
      for chunk in &chunks {
        let map = chunk.map.as_ref().unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, map);
        if matches!(format, OutputFormat::Esm)
          && !matches!(scenario, "render_chunk" | "intro" | "code_banner")
        {
          super::map_assertions::assert_unmapped(&chunk.code, map, "export");
        }
        std::fs::write(out_dir.join(chunk.filename.as_str()), &chunk.code).unwrap();
        std::fs::write(
          out_dir.join(chunk.sourcemap_filename.as_ref().unwrap()),
          map.to_json_string(),
        )
        .unwrap();
        if scenario == "comment_banner" {
          assert!(chunk.code.contains("/*! path-comment-banner */"));
        }
        captures.push(serde_json::json!({
          "pipeline": pipeline, "scenario": scenario, "mode": mode, "format": format.as_str(),
          "out_dir": out_dir, "filename": chunk.filename.as_str(), "is_entry": chunk.is_entry,
          "is_dynamic_entry": chunk.is_dynamic_entry,
          "imports": chunk.imports.iter().map(arcstr::ArcStr::as_str).collect::<Vec<_>>(),
          "dynamic_imports": chunk.dynamic_imports.iter().map(arcstr::ArcStr::as_str).collect::<Vec<_>>(),
          "code": chunk.code, "map": map.to_json_string(),
        }));
      }
      map_anchors += assert_anchors(&chunks, graph);
      // Compression must actually move the donor's expression across the module boundary.
      if mode != "mangle_only" && matches!(scenario, "cross_module" | "comment_banner") {
        let donor = entry_chunk
          .modules
          .keys
          .iter()
          .position(|id| id.as_str().ends_with("/cross/value.js"))
          .unwrap();
        assert!(
          entry_chunk.modules.values[donor].code().is_none(),
          "cross-module donor must be emptied"
        );
      }
      let runner = out_dir.join("execute.mjs");
      std::fs::write(&runner, format!(
        "import {{ createRequire }} from 'node:module';\nimport {{ readFileSync }} from 'node:fs';\nimport {{ runInThisContext }} from 'node:vm';\nglobalThis.__pathEvents = [];\nlet result;\n{}\nconsole.log(JSON.stringify({{ result: await result, events: globalThis.__pathEvents, hook: globalThis.__pathHook ?? null, intro: globalThis.__pathIntro ?? null, banner: globalThis.__pathBanner ?? null }}));\n",
        match format {
          OutputFormat::Esm => format!("result = (await import('./{}')).result;", entry_chunk.filename),
          OutputFormat::Cjs => format!("result = createRequire(import.meta.url)('./{}').result;", entry_chunk.filename),
          OutputFormat::Iife => format!("runInThisContext(readFileSync(new URL('./{}', import.meta.url), 'utf8')); result = PathCoverage.result;", entry_chunk.filename),
          OutputFormat::Umd => unreachable!(),
        }
      )).unwrap();
      let executed = Command::new("node").arg(&runner).current_dir(&out_dir).output().unwrap();
      assert!(
        executed.status.success(),
        "{pipeline}/{scenario}/{mode}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
      let actual: serde_json::Value = serde_json::from_slice(&executed.stdout).unwrap();
      let (result, events) = match graph {
        "cross_module" => (
          serde_json::json!({"value": "cross-inlined-value", "sum": 42}),
          serde_json::json!(["cross-dep-anchor", "cross-main-anchor"]),
        ),
        "private_fields" => (
          serde_json::json!(["first-value", "second-value", "inner-value:second-suffix"]),
          serde_json::json!([
            "private-first-anchor",
            "private-second-anchor",
            "private-main-anchor"
          ]),
        ),
        "splitting" => (
          serde_json::json!(["shared-value", "lazy:shared-value"]),
          serde_json::json!(["split-shared-anchor", "split-main-anchor", "split-lazy-anchor"]),
        ),
        _ => unreachable!(),
      };
      assert_eq!(
        actual,
        serde_json::json!({
          "result": result, "events": events,
          "hook": (scenario == "render_chunk").then_some(17),
          "intro": (scenario == "intro").then_some(19),
          "banner": (scenario == "code_banner").then_some(23),
        }),
        "{pipeline}/{scenario}/{mode}"
      );
      builds += 1;
      chunk_count += chunks.len();
    }
  }
  assert_eq!((builds, chunk_count, map_anchors), (27, 33, 60));
  if let Ok(path) = std::env::var("ROLLDOWN_PATH_MATRIX_OUTPUT") {
    std::fs::write(path, serde_json::to_string_pretty(&captures).unwrap()).unwrap();
  }
}

fn default_options() -> BundlerOptions {
  BundlerOptions {
    cwd: Some(FIXTURE.into()),
    input: Some(vec![InputItem { name: Some("main".into()), import: "./cross/main.js".into() }]),
    entry_filenames: Some("[name].mjs".to_string().into()),
    ..Default::default()
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn disabled_minify_allows_unsupported_chunks() {
  for format in [OutputFormat::Esm, OutputFormat::Cjs] {
    let output = Bundler::with_plugins(
      BundlerOptions {
        format: Some(format),
        minify: Some(RawMinifyOptions::Bool(false)),
        ..default_options()
      },
      vec![Plugin::new_shared(RenderChunkFallback)],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    let Output::Chunk(chunk) = &output.assets[0] else { panic!("expected JavaScript output") };
    assert!(chunk.code.contains("globalThis.__pathHook = 17;"));
    assert!(chunk.code.contains("cross-inlined-value"));
    assert_eq!(chunk.exports, ["result"]);
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn default_dce_is_supported() {
  let output =
    Bundler::new(BundlerOptions { sourcemap: Some(SourceMapType::File), ..default_options() })
      .unwrap()
      .generate()
      .await
      .unwrap();
  let chunks: Vec<_> = output
    .assets
    .iter()
    .filter_map(|output| match output {
      Output::Chunk(chunk) => Some(chunk.as_ref()),
      Output::Asset(_) => None,
    })
    .collect();
  assert_eq!(chunks.len(), 1);
  assert_eq!(assert_anchors(&chunks, "cross_module"), 2);
  let chunk = chunks[0];
  // Default DCE need not inline this donor. Successful generation and coverage prove the route;
  // retain output/export and map assertions without requiring full compression.
  assert_eq!(chunk.exports, ["result"]);
  super::map_assertions::assert_valid_map(&chunk.code, chunk.map.as_ref().unwrap());
  super::map_assertions::assert_unmapped(&chunk.code, chunk.map.as_ref().unwrap(), "export");
}

#[tokio::test(flavor = "multi_thread")]
async fn supports_whitespace_and_property_mangle() {
  for properties in [false, true] {
    let output = Bundler::new(BundlerOptions {
      sourcemap: Some(SourceMapType::File),
      minify: Some(RawMinifyOptions::Object(RawMinifyOptionsDetailed {
        // Keep compression enabled for property mangling so that its own gate is exercised.
        compress: properties.then(RawCompressOptions::default),
        mangle: None,
        mangle_properties: properties.then(|| {
          Box::new(RawManglePropertiesOptions {
            options: {
              let mut options =
                oxc::minifier::ManglePropertiesOptions::from_pattern("^sum$").unwrap();
              options.cache.insert("sum".into(), Some("cachedSum".into())).unwrap();
              options.cache.insert("value".into(), None).unwrap();
              options
            },
            patterns: ManglePropertiesPatterns {
              include: ManglePropertiesPattern { source: "^sum$".into(), flags: String::new() },
              exclude: None,
            },
          })
        }),
        remove_whitespace: true,
        ascii_only: false,
      })),
      ..default_options()
    })
    .unwrap()
    .generate()
    .await;
    let output = output.unwrap();
    assert_eq!(output.mangle_cache.is_some(), properties);
    let chunks: Vec<_> = output
      .assets
      .iter()
      .filter_map(
        |asset| {
          if let Output::Chunk(chunk) = asset { Some(chunk.as_ref()) } else { None }
        },
      )
      .collect();
    assert_eq!(assert_anchors(&chunks, "cross_module"), 2);
    let chunk = chunks[0];
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/property-ast")
      .join(properties.to_string());
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("main.mjs"), &chunk.code).unwrap();
    if properties {
      let cache = output.mangle_cache.as_ref().unwrap();
      assert_eq!(cache.get("sum"), Some(&Some("cachedSum".to_owned())));
      assert_eq!(cache.get("value"), Some(&None));
      assert!(chunk.code.contains("cachedSum"));
    }
    let runner = directory.join("execute.mjs");
    std::fs::write(&runner, format!("import assert from 'node:assert/strict';\nglobalThis.__pathEvents = [];\nconst {{ result }} = await import('./main.mjs');\nassert.equal(result.value, 'cross-inlined-value');\nassert.equal(result.{}, 42);\n", if properties { "cachedSum" } else { "sum" })).unwrap();
    let executed = Command::new("node").arg(runner).output().unwrap();
    assert!(executed.status.success(), "{}", String::from_utf8_lossy(&executed.stderr));
  }
}

#[derive(Debug)]
struct EmitNonJavaScriptAsset;

impl Plugin for EmitNonJavaScriptAsset {
  fn name(&self) -> Cow<'static, str> {
    "chunk-ast-non-javascript-asset".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::BuildStart
  }

  async fn build_start(
    &self,
    ctx: &PluginContext,
    _args: &rolldown_plugin::HookBuildStartArgs<'_>,
  ) -> rolldown_plugin::HookNoopReturn {
    ctx.emit_file(
      EmittedAsset {
        name: None,
        original_file_name: None,
        file_name: Some("untouched.txt".into()),
        source: "not JavaScript: } [".to_string().into(),
      },
      None,
      None,
    )?;
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn excludes_assets_and_declarations() {
  for extension in ["mjs", "d.ts", "d.cts", "d.mts"] {
    let declaration = extension != "mjs";
    let output = Bundler::with_plugins(
      BundlerOptions {
        entry_filenames: Some(format!("[name].{extension}").into()),
        // Declaration filenames skip minification even when their content is JavaScript.
        format: declaration.then_some(OutputFormat::Cjs),
        minify: Some(RawMinifyOptions::Bool(true)),
        ..default_options()
      },
      vec![Plugin::new_shared(EmitNonJavaScriptAsset)],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();
    assert_eq!(output.assets.len(), 2);
    let asset = output.assets.iter().find(|output| output.filename() == "untouched.txt").unwrap();
    assert!(matches!(asset, Output::Asset(_)));
    assert_eq!(asset.content_as_bytes(), b"not JavaScript: } [");
    let chunk = output
      .assets
      .iter()
      .find_map(|output| match output {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    assert_eq!(chunk.filename.as_str(), format!("main.{extension}"));
    assert!(chunk.code.contains("cross-inlined-value"));
  }
}
