use std::{
  borrow::Cow,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
};

use rolldown::{Bundler, BundlerOptions, InputItem};
use rolldown_common::{
  ChunkFilenamesOutputOption, EmittedAsset, EmittedPrebuiltChunk, ManglePropertiesPattern,
  ManglePropertiesPatterns, Output, RawCompressOptions, RawManglePropertiesOptions,
  RawMinifyOptions, RawMinifyOptionsDetailed,
};
use rolldown_error::{BatchedBuildDiagnostic, EventKind};
use rolldown_plugin::{HookGenerateBundleArgs, HookNoopReturn, HookUsage, Plugin, PluginContext};

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/path_coverage");

fn options(properties: bool) -> BundlerOptions {
  BundlerOptions {
    cwd: Some(FIXTURE.into()),
    input: Some(vec![InputItem { name: Some("main".into()), import: "./cross/main.js".into() }]),
    entry_filenames: Some("[name].mjs".to_string().into()),
    minify: Some(RawMinifyOptions::Object(RawMinifyOptionsDetailed {
      compress: Some(RawCompressOptions::default()),
      mangle: None,
      mangle_properties: properties.then(|| {
        let mut options = oxc::minifier::ManglePropertiesOptions::from_pattern("^sum$").unwrap();
        options.cache.insert("sum".into(), Some("cachedSum".into())).unwrap();
        options.cache.insert("value".into(), None).unwrap();
        Box::new(RawManglePropertiesOptions {
          options,
          patterns: ManglePropertiesPatterns {
            include: ManglePropertiesPattern { source: "^sum$".into(), flags: String::new() },
            exclude: None,
          },
        })
      }),
      remove_whitespace: true,
      ascii_only: false,
    })),
    ..Default::default()
  }
}

fn assert_multiple_chunks_error(errors: &BatchedBuildDiagnostic) {
  assert_eq!(errors.len(), 1, "{errors:?}");
  assert!(matches!(errors[0].kind(), EventKind::InvalidOptionError));
  assert_eq!(
    errors[0].to_string(),
    "Invalid value for option \"output.minify.mangleProps\" - property mangling is currently supported only when a build generates one JavaScript chunk. Use a single-chunk build or remove \"mangleProps\"."
  );
}

fn assert_property_cache(output: &rolldown::BundleOutput) {
  let cache = output.mangle_cache.as_ref().expect("the JavaScript chunk must return its cache");
  assert_eq!(cache.get("sum"), Some(&Some("cachedSum".to_owned())));
  assert_eq!(cache.get("value"), Some(&None));
  let chunk = output.assets.iter().find(|output| output.filename() == "main.mjs").unwrap();
  assert!(matches!(chunk, Output::Chunk(_)));
  assert!(std::str::from_utf8(chunk.content_as_bytes()).unwrap().contains("cachedSum"));
}

// The public property validation must run even when the chunk AST supplies a cache.
#[tokio::test(flavor = "multi_thread")]
async fn two_generated_javascript_chunks_require_single_chunk_property_mangling() {
  for properties in [false, true] {
    let result = Bundler::new(BundlerOptions {
      input: Some(vec![
        InputItem { name: Some("main".into()), import: "./cross/main.js".into() },
        InputItem { name: Some("other".into()), import: "./private/main.js".into() },
      ]),
      ..options(properties)
    })
    .unwrap()
    .generate()
    .await;
    if properties {
      assert_multiple_chunks_error(
        &result.err().expect("multiple JavaScript chunks must reject property mangling"),
      );
    } else {
      let output = result.unwrap();
      assert_eq!(output.assets.len(), 2);
      assert!(output.assets.iter().all(|output| matches!(output, Output::Chunk(_))));
      assert!(output.mangle_cache.is_none());
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn generated_declarations_do_not_count_as_javascript_chunks() {
  for extension in ["d.ts", "d.cts", "d.mts"] {
    for reverse in [false, true] {
      let mut input = vec![
        InputItem { name: Some("main".into()), import: "./cross/main.js".into() },
        InputItem { name: Some("types".into()), import: "./private/main.js".into() },
      ];
      if reverse {
        input.reverse();
      }
      let output = Bundler::new(BundlerOptions {
        input: Some(input),
        entry_filenames: Some(ChunkFilenamesOutputOption::Fn(Arc::new(move |chunk| {
          let filename = if chunk.name == "types" {
            format!("types.{extension}")
          } else {
            "main.mjs".to_owned()
          };
          Box::pin(async move { Ok(filename) })
        }))),
        ..options(true)
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      assert_eq!(output.assets.len(), 2);
      assert_property_cache(&output);
      let declaration = output
        .assets
        .iter()
        .find(|output| output.filename() == format!("types.{extension}"))
        .unwrap();
      assert!(matches!(declaration, Output::Chunk(_)));
      assert!(
        std::str::from_utf8(declaration.content_as_bytes())
          .unwrap()
          .contains("private-main-anchor")
      );
    }
  }
}

#[derive(Debug)]
struct EmitAtGenerateBundle {
  filename: &'static str,
  properties: bool,
  called: Arc<AtomicBool>,
}

const PREBUILT_JS: &str = "export const untouched = { sum: 7 };\n";
const PREBUILT_DECLARATION: &str = "export declare const untouched: { sum: number };\n";
const ASSET: &str = "not JavaScript: } [";

impl Plugin for EmitAtGenerateBundle {
  fn name(&self) -> Cow<'static, str> {
    "property-cache-generate-bundle".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::GenerateBundle
  }

  async fn generate_bundle(
    &self,
    ctx: &PluginContext,
    args: &mut HookGenerateBundleArgs<'_>,
  ) -> HookNoopReturn {
    // Prove that validation reached the late hook after minifying one ordinary JS chunk.
    assert_eq!(args.bundle.len(), 1);
    let Output::Chunk(chunk) = &args.bundle[0] else { panic!("expected JavaScript chunk") };
    assert_eq!(chunk.filename, "main.mjs");
    assert_eq!(chunk.code.contains("cachedSum"), self.properties);
    self.called.store(true, Ordering::Relaxed);
    ctx.file_emitter().emit_prebuilt_chunk(EmittedPrebuiltChunk {
      file_name: self.filename.into(),
      name: None,
      code: if self.filename == "prebuilt.mjs" { PREBUILT_JS } else { PREBUILT_DECLARATION }.into(),
      exports: vec!["untouched".into()],
      map: None,
      sourcemap_filename: None,
      facade_module_id: None,
      is_entry: false,
      is_dynamic_entry: false,
    });
    ctx.emit_file(
      EmittedAsset {
        name: None,
        original_file_name: None,
        file_name: Some("asset.js".into()),
        source: ASSET.to_owned().into(),
      },
      None,
      None,
    )?;
    Ok(())
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn generate_bundle_prebuilt_chunks_use_final_property_validation() {
  for filename in ["prebuilt.mjs", "prebuilt.d.ts", "prebuilt.d.cts", "prebuilt.d.mts"] {
    for properties in [false, true] {
      let called = Arc::new(AtomicBool::new(false));
      let result = Bundler::with_plugins(
        options(properties),
        vec![Plugin::new_shared(EmitAtGenerateBundle {
          filename,
          properties,
          called: Arc::clone(&called),
        })],
      )
      .unwrap()
      .generate()
      .await;
      assert!(called.load(Ordering::Relaxed), "generateBundle must run before final validation");
      if properties && filename == "prebuilt.mjs" {
        assert_multiple_chunks_error(
          &result.err().expect("multiple JavaScript chunks must reject property mangling"),
        );
        continue;
      }
      let output = result.unwrap();
      assert_eq!(output.assets.len(), 3);
      if properties {
        assert_property_cache(&output);
      } else {
        assert!(output.mangle_cache.is_none());
      }
      let prebuilt = output.assets.iter().find(|output| output.filename() == filename).unwrap();
      assert!(matches!(prebuilt, Output::Chunk(_)));
      assert_eq!(
        prebuilt.content_as_bytes(),
        if filename == "prebuilt.mjs" { PREBUILT_JS } else { PREBUILT_DECLARATION }.as_bytes()
      );
      let asset = output.assets.iter().find(|output| output.filename() == "asset.js").unwrap();
      assert!(matches!(asset, Output::Asset(_)));
      assert_eq!(asset.content_as_bytes(), ASSET.as_bytes());
    }
  }
}
