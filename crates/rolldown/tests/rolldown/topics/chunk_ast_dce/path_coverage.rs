use std::{borrow::Cow, path::Path, process::Command};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  AddonOutputOption, CommentsOptions, Output, OutputFormat, RawCompressOptions, RawMangleOptions,
  RawMinifyOptions, RawMinifyOptionsDetailed,
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

// Run this test in separate original/global processes: production options are cached globally.
// Path evidence is asserted by the external runner using ROLLDOWN_CHUNK_AST_COVERAGE, never by
// changing process-global environment variables while Tokio/Rayon workers may be using them.
#[tokio::test(flavor = "multi_thread")]
async fn execution_and_final_maps() {
  let global = std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global");
  let pipeline = if global { "global" } else { "original" };
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
      let output = Bundler::with_plugins(options, plugins).unwrap().generate().await.unwrap();
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
        if global
          && matches!(format, OutputFormat::Esm)
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
      if global && mode != "mangle_only" && matches!(scenario, "cross_module" | "comment_banner") {
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
