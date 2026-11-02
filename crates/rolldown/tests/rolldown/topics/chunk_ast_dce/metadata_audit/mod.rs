use std::{
  borrow::Cow,
  sync::{Arc, Mutex},
};

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::{AddonOutputOption, Modules, Output, RollupRenderedChunk};
use rolldown_plugin::{
  HookAugmentChunkHashReturn, HookGenerateBundleArgs, HookNoopReturn, HookRenderChunkArgs,
  HookRenderChunkOutput, HookRenderChunkReturn, HookTransformOutputMap, HookUsage, Plugin,
  PluginContext,
};

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/metadata_audit");
#[derive(Clone, Debug, PartialEq, Eq)]
struct ModuleMetadata {
  id: String,
  code: Option<String>,
  length: usize,
  exports: Vec<String>,
}

type Metadata = Vec<ModuleMetadata>;
type PreliminaryChunks = Arc<Mutex<Vec<(String, Arc<RollupRenderedChunk>)>>>;

fn short_id(id: &str) -> String {
  id.rsplit('/').next().unwrap().to_string()
}

fn metadata(modules: &Modules) -> Metadata {
  modules
    .keys
    .iter()
    .zip(&modules.values)
    .map(|(id, module)| {
      let code = module.code();
      let length = module.rendered_utf16_length();
      assert_eq!(length, code.as_deref().unwrap_or_default().encode_utf16().count());
      assert_eq!(module.rendered_byte_length(), code.as_deref().unwrap_or_default().len());
      let mut exports = module.rendered_exports.iter().map(ToString::to_string).collect::<Vec<_>>();
      exports.sort_unstable();
      ModuleMetadata { id: short_id(id.as_str()), code, length, exports }
    })
    .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
  stage: &'static str,
  modules: Metadata,
  code: Option<String>,
  filename: String,
  module_ids: Vec<String>,
  exports: Vec<String>,
}

type Observations = Arc<Mutex<Vec<Observation>>>;

fn observe(
  state: &Observations,
  stage: &'static str,
  chunk: &RollupRenderedChunk,
  code: Option<String>,
) {
  state.lock().unwrap().push(Observation {
    stage,
    modules: metadata(&chunk.modules),
    code,
    filename: chunk.filename.to_string(),
    module_ids: chunk.module_ids.iter().map(|id| short_id(id.as_str())).collect(),
    exports: chunk.exports.iter().map(ToString::to_string).collect(),
  });
}

#[derive(Debug)]
struct MetadataAudit {
  render_name: Option<&'static str>,
  augment: bool,
  state: Observations,
}

impl Plugin for MetadataAudit {
  fn name(&self) -> Cow<'static, str> {
    self.render_name.unwrap_or("metadata-audit").into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::AugmentChunkHash
      | HookUsage::GenerateBundle
      | if self.render_name.is_some() { HookUsage::RenderChunk } else { HookUsage::empty() }
  }

  async fn render_chunk(
    &self,
    _ctx: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    observe(&self.state, "renderChunk", &args.chunk, Some(args.code.to_string()));
    Ok(Some(HookRenderChunkOutput {
      code: format!("{}\nconst auditUnused = 123;\n", args.code),
      map: HookTransformOutputMap::default(),
    }))
  }

  async fn augment_chunk_hash(
    &self,
    _ctx: &PluginContext,
    chunk: Arc<RollupRenderedChunk>,
  ) -> HookAugmentChunkHashReturn {
    observe(&self.state, "augmentChunkHash", &chunk, None);
    Ok(self.augment.then(|| format!("{:?}", metadata(&chunk.modules))))
  }

  async fn generate_bundle(
    &self,
    _ctx: &PluginContext,
    args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    let chunk = args
      .bundle
      .iter()
      .find_map(|output| match output {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    self.state.lock().unwrap().push(Observation {
      stage: "generateBundle",
      modules: metadata(&chunk.modules),
      code: Some(chunk.code.clone()),
      filename: chunk.filename.to_string(),
      module_ids: chunk.module_ids.iter().map(|id| short_id(id.as_str())).collect(),
      exports: chunk.exports.iter().map(ToString::to_string).collect(),
    });
    Ok(())
  }
}

// Metadata describes the printed segment, while IDs/exports keep their link-stage identity.
#[tokio::test(flavor = "multi_thread")]
async fn metadata_and_hook_order() {
  for full in [false, true] {
    for (render_name, augment) in [
      (None, false),
      (None, true),
      (Some("ordinary-metadata-audit"), false),
      (Some("builtin:asset-module"), false),
      (Some("builtin:copy-module"), false),
      (Some("rolldown-testing:hidden-module-markers"), false),
    ] {
      let state: Observations = Arc::default();
      let post_state = Arc::clone(&state);
      let mut bundler = Bundler::with_plugins(
        BundlerOptions {
          cwd: Some(FIXTURE.into()),
          input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
          entry_filenames: Some("[name]-[hash].mjs".to_string().into()),
          minify: full.then(|| true.into()),
          post_banner: Some(AddonOutputOption::Fn(Arc::new(move |chunk| {
            observe(&post_state, "postBanner", &chunk, None);
            Box::pin(async { Ok(Some("/* metadata audit post banner */".into())) })
          }))),
          ..Default::default()
        },
        vec![Plugin::new_shared(MetadataAudit { render_name, augment, state: Arc::clone(&state) })],
      )
      .unwrap();
      let output = bundler.generate().await;
      let output = output.unwrap();
      let observations = state.lock().unwrap();
      let stages: Vec<_> = observations.iter().map(|observation| observation.stage).collect();
      let expected = if render_name.is_some() {
        vec!["postBanner", "renderChunk", "augmentChunkHash", "generateBundle"]
      } else {
        vec!["postBanner", "augmentChunkHash", "generateBundle"]
      };
      assert_eq!(stages, expected);
      for observation in observations.iter() {
        assert_eq!(observation.modules, observations[0].modules);
        assert_eq!(observation.module_ids, observations[0].module_ids);
        assert_eq!(observation.exports, ["result"]);
      }
      // Text hooks observe pre-optimization fragments regardless of their names. Their
      // appended declarations enter the shared optimizer after all hooks finish.
      let eligible = render_name.is_none();
      let dep = observations[0].modules.iter().find(|module| module.id == "dep.js").unwrap();
      let main = observations[0].modules.iter().find(|module| module.id == "main.js").unwrap();
      assert_eq!(dep.code.is_none(), eligible);
      assert_eq!(dep.exports, ["value"]);
      assert_eq!(main.exports, ["result"]);
      // Even an empty segment keeps its module ID and retained original export names.
      for id in ["dep.js", "main.js"] {
        assert!(observations[0].module_ids.iter().any(|module| module == id));
      }
      if eligible {
        assert_eq!(dep.length, 0);
        assert!(main.code.as_ref().unwrap().contains("metadata-marker"));
      } else {
        assert!(dep.code.as_ref().unwrap().contains("metadata-marker"));
      }
      let marker_owner = if eligible { main } else { dep };
      // The Unicode marker protects the public String.length unit after code movement.
      let marker_code = marker_owner.code.as_ref().unwrap();
      assert!(marker_code.contains('😀'));
      assert_ne!(marker_owner.length, marker_code.len());
      if render_name.is_some() {
        assert!(!observations[1].code.as_ref().unwrap().contains("metadata audit post banner"));
      }
      let last = observations.last().unwrap();
      let final_code = last.code.as_ref().unwrap();
      assert!(final_code.starts_with("/* metadata audit post banner */"));
      assert!(!last.filename.contains("!~{"));
      assert!(observations[0].filename.contains("!~{"));
      if render_name.is_some() {
        assert!(!final_code.contains("auditUnused"));
      }
      let returned = output
        .assets
        .iter()
        .find_map(|output| match output {
          Output::Chunk(chunk) => Some(chunk),
          Output::Asset(_) => None,
        })
        .unwrap();
      assert_eq!(metadata(&returned.modules), last.modules);
      assert_eq!(
        returned.module_ids.iter().map(|id| short_id(id.as_str())).collect::<Vec<_>>(),
        last.module_ids
      );
      assert_eq!(
        returned.exports.iter().map(ToString::to_string).collect::<Vec<_>>(),
        last.exports
      );
      // Export glue belongs to the generated tail, not any module's code/length.
      assert!(returned.code.contains("export"));
      for module in &last.modules {
        assert!(!module.code.as_deref().unwrap_or_default().contains("export "));
        assert!(!module.code.as_deref().unwrap_or_default().contains("unused-metadata-marker"));
      }
    }
  }
}

#[derive(Debug)]
struct RetainedMetadataAudit {
  preliminary: PreliminaryChunks,
  final_modules: Arc<Mutex<Vec<(String, Metadata)>>>,
}
impl Plugin for RetainedMetadataAudit {
  fn name(&self) -> Cow<'static, str> {
    "retained-metadata".into()
  }
  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::RenderChunk | HookUsage::AugmentChunkHash | HookUsage::GenerateBundle
  }
  async fn render_chunk(
    &self,
    _: &PluginContext,
    args: &HookRenderChunkArgs<'_>,
  ) -> HookRenderChunkReturn {
    self.preliminary.lock().unwrap().push((args.chunk.name.to_string(), Arc::clone(&args.chunk)));
    Ok(None)
  }
  async fn augment_chunk_hash(
    &self,
    _: &PluginContext,
    chunk: Arc<RollupRenderedChunk>,
  ) -> HookAugmentChunkHashReturn {
    self.preliminary.lock().unwrap().push((chunk.name.to_string(), chunk));
    Ok(None)
  }
  async fn generate_bundle(
    &self,
    _: &PluginContext,
    args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    for output in args.bundle.iter() {
      if let Output::Chunk(chunk) = output {
        self.final_modules.lock().unwrap().push((chunk.name.to_string(), metadata(&chunk.modules)));
      }
    }
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn retained_program_final_metadata_uses_compressed_fragments() {
  use rolldown_common::{OutputFormat, RawMinifyOptions};
  for format in [OutputFormat::Esm, OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    for full in [false, true] {
      let preliminary = Arc::default();
      let final_modules = Arc::default();
      let output = Bundler::with_plugins(
        BundlerOptions {
          cwd: Some(FIXTURE.into()),
          input: Some(vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]),
          name: Some("Metadata".into()),
          format: Some(format),
          minify: Some(if full {
            RawMinifyOptions::Bool(true)
          } else {
            RawMinifyOptions::DeadCodeEliminationOnly
          }),
          ..Default::default()
        },
        vec![Plugin::new_shared(RetainedMetadataAudit {
          preliminary: Arc::clone(&preliminary),
          final_modules: Arc::clone(&final_modules),
        })],
      )
      .unwrap()
      .generate()
      .await
      .unwrap();
      let snapshots = preliminary.lock().unwrap();
      assert_eq!(snapshots.len(), 2);
      let before = metadata(&snapshots[0].1.modules);
      assert_eq!(before, metadata(&snapshots[1].1.modules));
      assert!(
        before
          .iter()
          .find(|m| m.id == "dep.js")
          .unwrap()
          .code
          .as_deref()
          .unwrap()
          .contains("metadata-marker")
      );
      let returned = output
        .assets
        .iter()
        .find_map(|output| if let Output::Chunk(chunk) = output { Some(chunk) } else { None })
        .unwrap();
      let after = metadata(&returned.modules);
      assert_eq!(after, final_modules.lock().unwrap()[0].1);
      if full {
        assert!(
          after.iter().find(|m| m.id == "dep.js").unwrap().code.is_none(),
          "{format:?}: {after:?}"
        );
        let main = after.iter().find(|m| m.id == "main.js").unwrap();
        assert!(main.code.as_deref().unwrap().contains("metadata-marker"));
        assert!(main.code.as_deref().unwrap().contains('😀'));
        assert!(!main.code.as_deref().unwrap().contains("Object.define"));
      }
      // Plugins can retain either preliminary Arc: final reconciliation must use copy-on-write.
      assert_eq!(before, metadata(&snapshots[0].1.modules));
      assert_eq!(before, metadata(&snapshots[1].1.modules));
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn inline_and_dev_programs_exclude_generated_glue_from_final_metadata() {
  use rolldown::{DevModeOptions, ExperimentalOptions, SourceMapType};
  use rolldown_common::{
    CodeSplittingMode, InlineCommonChunksOptions, ManualCodeSplittingOptions,
    PreserveEntrySignatures, RawMinifyOptions,
  };

  for inline in [true, false] {
    let pipeline = "chunk-ast";
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .join("../../tmp/adoption-audit/metadata-retained")
      .join(pipeline)
      .join(if inline { "inline" } else { "dev" });
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
      directory.join("dep.js"),
      "export const value = { marker: 'metadata-marker-😀' };\n",
    )
    .unwrap();
    std::fs::write(
      directory.join("main.js"),
      "import { value } from './dep.js';\nexport const result = { value };\n",
    )
    .unwrap();
    let input = if inline {
      ["a", "b"]
        .into_iter()
        .map(|name| {
          std::fs::write(
            directory.join(format!("{name}.js")),
            format!("import {{ result }} from './main.js';\nglobalThis.{name} = result;\n"),
          )
          .unwrap();
          InputItem { name: Some(name.into()), import: format!("./{name}.js") }
        })
        .collect()
    } else {
      vec![InputItem { name: Some("main".into()), import: "./main.js".into() }]
    };
    let preliminary = Arc::default();
    let final_modules = Arc::default();
    let output = Bundler::with_plugins(
      BundlerOptions {
        cwd: Some(directory),
        input: Some(input),
        code_splitting: inline.then(|| {
          CodeSplittingMode::Advanced(ManualCodeSplittingOptions {
            experimental_inline_common_chunks: Some(InlineCommonChunksOptions {
              max_size: Some(f64::INFINITY),
              exclude: None,
            }),
            ..Default::default()
          })
        }),
        experimental: (!inline).then(|| ExperimentalOptions {
          dev_mode: Some(DevModeOptions::default()),
          ..Default::default()
        }),
        minify: Some(RawMinifyOptions::Bool(true)),
        preserve_entry_signatures: Some(PreserveEntrySignatures::False),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      },
      vec![Plugin::new_shared(RetainedMetadataAudit {
        preliminary: Arc::clone(&preliminary),
        final_modules: Arc::clone(&final_modules),
      })],
    )
    .unwrap()
    .generate()
    .await
    .unwrap();

    let snapshots = preliminary.lock().unwrap();
    let generated = if inline { "__share(" } else { "registerGraph(" };
    let mut carriers = 0;
    for asset in &output.assets {
      let Output::Chunk(returned) = asset else { continue };
      let name = returned.name.as_str();
      let chunk_snapshots: Vec<_> =
        snapshots.iter().filter(|(snapshot_name, _)| snapshot_name == name).collect();
      assert_eq!(chunk_snapshots.len(), 2, "{name}");
      let before = metadata(&chunk_snapshots[0].1.modules);
      assert_eq!(before, metadata(&chunk_snapshots[1].1.modules));
      let Some(dep) = before.iter().find(|module| module.id == "dep.js") else {
        continue;
      };
      carriers += 1;
      assert!(dep.code.as_deref().unwrap().contains("metadata-marker"));
      let after = metadata(&returned.modules);
      assert_eq!(
        after,
        final_modules.lock().unwrap().iter().find(|(final_name, _)| final_name == name).unwrap().1
      );
      assert_ne!(after, before);
      let dep = after.iter().find(|module| module.id == "dep.js").unwrap();
      assert!(dep.code.as_deref().unwrap().contains("metadata-marker"));
      assert!(after.iter().any(|module| module.code.as_deref().unwrap_or_default().contains('😀')));
      for module in &after {
        assert!(!module.code.as_deref().unwrap_or_default().contains(generated));
      }
      assert_eq!(before, metadata(&chunk_snapshots[0].1.modules));
      assert_eq!(before, metadata(&chunk_snapshots[1].1.modules));
    }
    assert!(carriers >= if inline { 2 } else { 1 }, "{inline}: {carriers}");
  }
}
