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
#[derive(Debug, PartialEq, Eq)]
struct ModuleMetadata {
  id: String,
  code: Option<String>,
  length: usize,
  exports: Vec<String>,
}

type Metadata = Vec<ModuleMetadata>;

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
// Run in separate processes with global mode enabled/unset to cover both rendering pipelines.
#[tokio::test(flavor = "multi_thread")]
async fn metadata_and_hook_order() {
  let global = std::env::var("ROLLDOWN_CHUNK_AST_DCE").is_ok_and(|mode| mode == "global");
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
      let output = bundler.generate().await.unwrap();
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
      // Every custom renderChunk plugin must fall back, including names used by builtins
      // and the harness marker plugin. Its appended declaration still needs the minifier.
      let eligible = global && render_name.is_none();
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
