use std::{
  io::Write,
  process::{Command, Stdio},
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::{
  AddonOutputOption, CommentsOptions, GlobalsOutputOption, Output, OutputFormat,
  RawCompressOptions, RawMangleOptions, RawMinifyOptions, RawMinifyOptionsDetailed,
};

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/wrapped_formats");

fn minify(mode: &str) -> RawMinifyOptions {
  match mode {
    "dce" => RawMinifyOptions::DeadCodeEliminationOnly,
    "full" => RawMinifyOptions::Bool(true),
    _ => RawMinifyOptions::Object(RawMinifyOptionsDetailed {
      compress: (mode == "compress").then(RawCompressOptions::default),
      mangle: (mode == "mangle").then(RawMangleOptions::default),
      mangle_properties: None,
      remove_whitespace: mode == "mangle",
      ascii_only: false,
    }),
  }
}

async fn check_wrapped_formats(entry: &str, extend: bool) {
  let (content, marker) = match entry {
    "named" => (include_str!("wrapped_formats/named.js"), "named-wrapper-anchor"),
    "default" => (include_str!("wrapped_formats/default.js"), "default-wrapper-anchor"),
    "none" => (include_str!("wrapped_formats/none.js"), "no-exports-wrapper-anchor"),
    _ => unreachable!(),
  };
  for format in [OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "compress", "mangle", "full"] {
      let calls = Arc::new(AtomicUsize::new(0));
      let callback_calls = Arc::clone(&calls);
      let output = Bundler::new(BundlerOptions {
        cwd: Some(FIXTURE.into()),
        input: Some(vec![InputItem { name: Some("main".into()), import: format!("./{entry}.js") }]),
        external: Some(vec!["wrapped-external".into()].into()),
        globals: Some(GlobalsOutputOption::Fn(Arc::new(move |id| {
          assert_eq!(id, "wrapped-external");
          callback_calls.fetch_add(1, Ordering::Relaxed);
          Box::pin(async { Ok("Dependencies.wrapped".into()) })
        }))),
        format: Some(format),
        name: (entry != "none")
          .then(|| if extend { "Wrapped.nested.api".into() } else { "Wrapped".into() }),
        extend: Some(extend),
        entry_filenames: Some("main.js".to_string().into()),
        minify: Some(minify(mode)),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap_or_else(|error| panic!("{format:?}/{entry}/{extend}/{mode}: {error}"));
      assert_eq!(calls.load(Ordering::Relaxed), 1, "globals must run once per external");
      let chunks: Vec<_> = output
        .assets
        .iter()
        .filter_map(|asset| match asset {
          Output::Chunk(chunk) => Some(chunk),
          Output::Asset(_) => None,
        })
        .collect();
      assert_eq!(chunks.len(), 1);
      let chunk = chunks[0];
      let serialized = chunk.map.as_ref().unwrap().to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
      super::map_assertions::assert_valid_map(&chunk.code, &map);
      for (anchor, source, source_content) in [
        (marker, format!("{entry}.js"), content),
        ("private-wrapper-anchor", "private.js".into(), include_str!("wrapped_formats/private.js")),
      ] {
        super::map_assertions::assert_mapping(
          &chunk.code,
          &map,
          anchor,
          &source,
          source_content,
          &format!("'{anchor}'"),
        );
      }
      // The final curly closes generated wrapper glue (or its generated exports argument).
      // Inspect its exact position, including on the same line as minified user code.
      let (line, column) =
        super::map_assertions::position(&chunk.code, chunk.code.rfind('}').unwrap());
      let lookup = map.generate_lookup_table();
      let token = map.lookup_source_view_token(&lookup, line, column);
      assert!(
        token.is_none_or(|token| token.get_source_id().is_none()),
        "wrapper boundary must be unmapped ({format:?}/{entry}/{extend}/{mode}): {token:?}"
      );
      let mut child = Command::new("node")
        .args(["--input-type=module", "--eval", include_str!("wrapped_formats/execute.mjs")])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
      let input = serde_json::json!({
        "code": chunk.code,
        "format": format.as_str(),
        "entry": entry,
        "extend": extend,
      });
      child.stdin.take().unwrap().write_all(input.to_string().as_bytes()).unwrap();
      let executed = child.wait_with_output().unwrap();
      assert!(
        executed.status.success(),
        "{format:?}/{entry}/{extend}/{mode}: {}",
        String::from_utf8_lossy(&executed.stderr)
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn wrapper_comments_preserve_hashbang_and_addon_policy() {
  for format in [OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "compress", "full"] {
      for legal in [false, true] {
        let output = Bundler::new(BundlerOptions {
          cwd: Some(FIXTURE.into()),
          input: Some(vec![InputItem {
            name: Some("main".into()),
            import: "./hashbang.js".into(),
          }]),
          external: Some(vec!["wrapped-external".into()].into()),
          globals: Some(GlobalsOutputOption::FxHashMap(rustc_hash::FxHashMap::from_iter([(
            "wrapped-external".into(),
            "Dependency".into(),
          )]))),
          format: Some(format),
          minify: Some(minify(mode)),
          comments: Some(CommentsOptions { legal, annotation: false, jsdoc: false }),
          banner: Some(AddonOutputOption::String(Some(
            "// banner-marker\n/*! banner-license-marker */".into(),
          ))),
          footer: Some(AddonOutputOption::String(Some(
            "/* footer-marker */\n/*! footer-license-marker */".into(),
          ))),
          sourcemap: Some(SourceMapType::File),
          ..Default::default()
        })
        .unwrap()
        .generate()
        .await
        .unwrap();
        let chunk = output
          .assets
          .iter()
          .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
          .unwrap();
        if matches!(format, OutputFormat::Iife) {
          assert!(
            chunk.code.starts_with("#!/usr/bin/env node\n"),
            "{format:?}/{mode}/{legal}: {}",
            chunk.code
          );
        } else {
          assert!(!chunk.code.contains("#!"));
        }
        let whitespace = mode == "full";
        for marker in ["banner-marker", "footer-marker"] {
          assert_eq!(
            chunk.code.contains(marker),
            !whitespace,
            "{format:?}/{mode}/{legal}/{marker}: {}",
            chunk.code
          );
        }
        for marker in ["banner-license-marker", "footer-license-marker"] {
          assert_eq!(
            chunk.code.contains(marker),
            legal || !whitespace,
            "{format:?}/{mode}/{legal}/{marker}: {}",
            chunk.code
          );
        }
        assert_eq!(chunk.code.contains("module-license-marker"), legal);
        assert!(!chunk.code.contains("module-normal-marker"));
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        let mut child = Command::new("node").args(["--input-type=module", "--eval", "import vm from 'node:vm'; import assert from 'node:assert/strict'; let code = ''; for await (const part of process.stdin) code += part; const context = { Dependency: { value: 42 } }; vm.runInNewContext(code, context); assert.equal(context.__wrappedValue, 42);"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(chunk.code.as_bytes()).unwrap();
        let execution = child.wait_with_output().unwrap();
        assert!(execution.status.success(), "{}", String::from_utf8_lossy(&execution.stderr));
      }
    }
  }
}

const ADDONS: [&str; 4] = [
  "globalThis.__addonEvents.push('banner');",
  "(function () { globalThis.__addonEvents.push('intro');",
  "globalThis.__addonEvents.push('outro'); })();",
  "globalThis.__addonEvents.push('footer');",
];

#[derive(Debug)]
struct AddonHooks {
  calls: Arc<[AtomicUsize; 4]>,
}

impl rolldown_plugin::Plugin for AddonHooks {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "lexical-addons".into()
  }

  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    use rolldown_plugin::HookUsage;
    HookUsage::Banner | HookUsage::Intro | HookUsage::Outro | HookUsage::Footer
  }

  async fn banner(
    &self,
    _: &rolldown_plugin::PluginContext,
    _: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    self.calls[0].fetch_add(1, Ordering::Relaxed);
    Ok(Some(ADDONS[0].into()))
  }

  async fn intro(
    &self,
    _: &rolldown_plugin::PluginContext,
    _: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    self.calls[1].fetch_add(1, Ordering::Relaxed);
    Ok(Some(ADDONS[1].into()))
  }

  async fn outro(
    &self,
    _: &rolldown_plugin::PluginContext,
    _: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    self.calls[2].fetch_add(1, Ordering::Relaxed);
    Ok(Some(ADDONS[2].into()))
  }

  async fn footer(
    &self,
    _: &rolldown_plugin::PluginContext,
    _: &rolldown_plugin::HookAddonArgs,
  ) -> rolldown_plugin::HookInjectionOutputReturn {
    self.calls[3].fetch_add(1, Ordering::Relaxed);
    Ok(Some(ADDONS[3].into()))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn addons_share_lexical_scope_with_module_bodies() {
  for format in [OutputFormat::Esm, OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "compress", "mangle", "full"] {
      for provider in ["string", "callback", "hook"] {
        let calls = Arc::new(std::array::from_fn::<_, 4, _>(|_| AtomicUsize::new(0)));
        let addon = |index: usize| {
          if provider == "callback" {
            let calls = Arc::clone(&calls);
            Some(AddonOutputOption::Fn(Arc::new(move |_| {
              calls[index].fetch_add(1, Ordering::Relaxed);
              Box::pin(async move { Ok(Some(ADDONS[index].into())) })
            })))
          } else if provider == "string" {
            Some(AddonOutputOption::String(Some(ADDONS[index].into())))
          } else {
            None
          }
        };
        let plugins = if provider == "hook" {
          vec![rolldown_plugin::Plugin::new_shared(AddonHooks { calls: Arc::clone(&calls) })]
        } else {
          vec![]
        };
        let output = Bundler::with_plugins(
          BundlerOptions {
            cwd: Some(FIXTURE.into()),
            input: Some(vec![InputItem {
              name: Some("main".into()),
              import: "./addons.js".into(),
            }]),
            format: Some(format),
            minify: Some(minify(mode)),
            banner: addon(0),
            intro: addon(1),
            outro: addon(2),
            footer: addon(3),
            sourcemap: Some(SourceMapType::File),
            ..Default::default()
          },
          plugins,
        )
        .unwrap()
        .generate()
        .await
        .unwrap();
        for count in calls.iter() {
          assert_eq!(
            count.load(Ordering::Relaxed),
            usize::from(provider != "string"),
            "{format:?}/{mode}/{provider}"
          );
        }
        let chunk = output
          .assets
          .iter()
          .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
          .unwrap();
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        super::map_assertions::assert_mapping(
          &chunk.code,
          &map,
          "module-addon-anchor",
          "addons.js",
          include_str!("wrapped_formats/addons.js"),
          "'module-addon-anchor'",
        );
        super::map_assertions::assert_unmapped(&chunk.code, &map, "banner");
        let mut child = Command::new("node").args(["--input-type=module", "--eval", "import assert from 'node:assert/strict'; let code = ''; for await (const part of process.stdin) code += part; globalThis.__addonEvents = []; await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64')); assert.deepEqual(globalThis.__addonEvents, ['banner', 'intro', 'module-addon-anchor', 'outro', 'footer']); assert.equal(globalThis.__addonRead(), 17); assert.equal(globalThis.captured, undefined);"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(chunk.code.as_bytes()).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
          result.status.success(),
          "{format:?}/{mode}/{provider}: {}",
          String::from_utf8_lossy(&result.stderr)
        );
      }
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn addons_can_control_the_first_module_statement() {
  for format in [OutputFormat::Esm, OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "compress", "mangle", "full"] {
      for comment_body in [false, true] {
        let output = Bundler::new(BundlerOptions {
          cwd: Some(FIXTURE.into()),
          input: Some(vec![InputItem { name: Some("main".into()), import: "./addons.js".into() }]),
          format: Some(format),
          minify: Some(minify(mode)),
          intro: Some(AddonOutputOption::String(Some(
            if comment_body { "/*" } else { "if (false)" }.into(),
          ))),
          outro: comment_body.then(|| AddonOutputOption::String(Some("*/".into()))),
          sourcemap: Some(SourceMapType::File),
          ..Default::default()
        })
        .unwrap()
        .generate()
        .await
        .unwrap();
        let chunk = output
          .assets
          .iter()
          .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
          .unwrap();
        let serialized = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&serialized).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        let mut child = Command::new("node")
          .args(["--input-type=module", "--eval", &format!("import assert from 'node:assert/strict'; let code = ''; for await (const part of process.stdin) code += part; globalThis.__addonEvents = []; await import('data:text/javascript;base64,' + Buffer.from(code).toString('base64')); assert.deepEqual(globalThis.__addonEvents, []); assert.equal(typeof globalThis.__addonRead, '{}');", if comment_body { "undefined" } else { "function" })])
          .stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(chunk.code.as_bytes()).unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(
          result.status.success(),
          "{format:?}/{mode}/{comment_body}: {}",
          String::from_utf8_lossy(&result.stderr)
        );
      }
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn invalid_addons_return_parse_diagnostics() {
  for format in [OutputFormat::Esm, OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    let errors = Bundler::new(BundlerOptions {
      cwd: Some(FIXTURE.into()),
      input: Some(vec![InputItem { name: Some("main".into()), import: "./addons.js".into() }]),
      format: Some(format),
      minify: Some(RawMinifyOptions::Bool(true)),
      intro: Some(AddonOutputOption::String(Some("const = ;".into()))),
      ..Default::default()
    })
    .unwrap()
    .generate()
    .await
    .err()
    .expect("invalid addon must return diagnostics");
    assert!(!errors.is_empty(), "{format:?}");
    assert!(
      errors.iter().all(|error| matches!(error.kind(), rolldown_error::EventKind::ParseError)),
      "{format:?}: {errors:?}"
    );
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn wrapper_globals_accept_expressions_and_namespace_properties() {
  for format in [OutputFormat::Iife, OutputFormat::Umd] {
    for extend in [false, true] {
      for mode in ["dce", "full"] {
        let calls = Arc::new(AtomicUsize::new(0));
        let callback_calls = Arc::clone(&calls);
        let output = Bundler::new(BundlerOptions {
          cwd: Some(FIXTURE.into()),
          input: Some(vec![InputItem { name: Some("main".into()), import: "./named.js".into() }]),
          external: Some(vec!["wrapped-external".into()].into()),
          globals: Some(GlobalsOutputOption::Fn(Arc::new(move |_| {
            callback_calls.fetch_add(1, Ordering::Relaxed);
            Box::pin(async move {
              Ok(
                if matches!(format, OutputFormat::Iife) {
                  "(globalThis.__globalReads++, Dependencies['wrapped-value'])"
                } else {
                  "Dependencies.wrapped-value"
                }
                .into(),
              )
            })
          }))),
          format: Some(format),
          name: Some("包裹.1-api".into()),
          extend: Some(extend),
          es_module: Some(rolldown_common::EsModuleFlag::Always),
          generated_code: Some(rolldown_common::GeneratedCodeOptions { symbols: true }),
          minify: Some(minify(mode)),
          sourcemap: Some(SourceMapType::File),
          ..Default::default()
        })
        .unwrap()
        .generate()
        .await
        .unwrap();
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let chunk = output
          .assets
          .iter()
          .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
          .unwrap();
        let json = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        let mut child = Command::new("node").args(["--input-type=module", "--eval", "import vm from 'node:vm'; import assert from 'node:assert/strict'; let input = ''; for await (const part of process.stdin) input += part; const { code, extend, iife } = JSON.parse(input); const existing = { retained: 19 }; const external = Object.assign(value => value + 2, { offset: 3 }); const sandbox = { __globalReads: 0, __wrappedInput: 40, __wrappedEvents: [], Dependencies: { 'wrapped-value': external }, '包裹': { '1-api': existing, sibling: 23 } }; vm.runInNewContext(code, sandbox); const result = sandbox['包裹']['1-api']; assert.equal(result === existing, extend); assert.equal(sandbox['包裹'].sibling, 23); assert.equal(result.retained, extend ? 19 : undefined); assert.equal(result.current, 1); result.increment(); assert.equal(result.current, 2); assert.equal(result.read().value, 42); assert.equal(Object.prototype.toString.call(result), '[object Module]'); assert.equal(result.__esModule, true); assert.equal(sandbox.__globalReads, iife ? 1 : 0);"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(serde_json::json!({ "code": chunk.code, "extend": extend, "iife": matches!(format, OutputFormat::Iife) }).to_string().as_bytes()).unwrap();
        let execution = child.wait_with_output().unwrap();
        assert!(
          execution.status.success(),
          "{format:?}/{extend}/{mode}: {}",
          String::from_utf8_lossy(&execution.stderr)
        );
      }
    }
  }
}

#[derive(Debug)]
struct InjectWrapperInput(&'static str);

impl rolldown_plugin::Plugin for InjectWrapperInput {
  fn name(&self) -> std::borrow::Cow<'static, str> {
    "escaped-directive".into()
  }
  fn register_hook_usage(&self) -> rolldown_plugin::HookUsage {
    rolldown_plugin::HookUsage::Transform
  }
  async fn transform(
    &self,
    _: rolldown_plugin::SharedTransformPluginContext,
    args: &rolldown_plugin::HookTransformArgs<'_>,
  ) -> rolldown_plugin::HookTransformReturn {
    if !args.id.ends_with("/addons.js") {
      return Ok(None);
    }
    Ok(Some(rolldown_plugin::HookTransformOutput {
      code: Some(self.0.into()),
      ..Default::default()
    }))
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn escaped_directive_does_not_enable_strict_mode() {
  for format in [OutputFormat::Cjs, OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "full"] {
      let output = Bundler::new(BundlerOptions {
        cwd: Some(FIXTURE.into()),
        input: Some(vec![InputItem {
          name: Some("main".into()),
          import: "./escaped-directive.cjs".into(),
        }]),
        format: Some(format),
        minify: Some(minify(mode)),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      let chunk = output
        .assets
        .iter()
        .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
        .unwrap();
      let mut child = Command::new("node").args(["--input-type=module", "--eval", "import vm from 'node:vm'; import assert from 'node:assert/strict'; let code = ''; for await (const part of process.stdin) code += part; const sandbox = {}; vm.runInNewContext(code, sandbox); assert.equal(sandbox.__directiveResult, false);"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
      child.stdin.take().unwrap().write_all(chunk.code.as_bytes()).unwrap();
      let execution = child.wait_with_output().unwrap();
      assert!(
        execution.status.success(),
        "{format:?}/{mode}: {}\n{}",
        String::from_utf8_lossy(&execution.stderr),
        chunk.code
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn cjs_default_factory_runs_once() {
  for format in [OutputFormat::Iife, OutputFormat::Umd] {
    for mode in ["dce", "full"] {
      for exported in [false, true] {
        let output = Bundler::with_plugins(
          BundlerOptions {
            cwd: Some(FIXTURE.into()),
            input: Some(vec![InputItem {
              name: Some("main".into()),
              import: "./addons.js".into(),
            }]),
            format: Some(format),
            name: Some("Factory.result".into()),
            exports: Some(if exported {
              rolldown_common::OutputExports::Default
            } else {
              rolldown_common::OutputExports::None
            }),
            minify: Some(minify(mode)),
            sourcemap: Some(SourceMapType::File),
            ..Default::default()
          },
          vec![rolldown_plugin::Plugin::new_shared(InjectWrapperInput(
            "globalThis.__factoryRuns++; module.exports = { value: 37 };",
          ))],
        )
        .unwrap()
        .generate()
        .await;
        if !exported {
          let errors = output.err().expect("CJS default cannot use exports none");
          assert!(errors.iter().any(|error| matches!(
            error.kind(),
            rolldown_error::EventKind::InvalidExportOptionError
          )));
          continue;
        }
        let output = output.unwrap();
        let chunk = output
          .assets
          .iter()
          .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
          .unwrap();
        let json = chunk.map.as_ref().unwrap().to_json_string();
        let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
        super::map_assertions::assert_valid_map(&chunk.code, &map);
        let mut child = Command::new("node").args(["--input-type=module", "--eval", "import vm from 'node:vm'; import assert from 'node:assert/strict'; let input = ''; for await (const part of process.stdin) input += part; const { code, umd, exported } = JSON.parse(input); for (const branch of umd && exported ? ['browser', 'cjs', 'amd'] : ['browser']) { const sandbox = { __factoryRuns: 0 }; let result; if (branch === 'cjs') { sandbox.exports = {}; sandbox.module = { exports: sandbox.exports }; } if (branch === 'amd') { sandbox.define = (deps, factory) => { assert.equal(deps.length, 0); result = factory(); }; sandbox.define.amd = {}; } vm.runInNewContext(code, sandbox); if (exported) { if (branch === 'browser') result = sandbox.Factory.result; else if (branch === 'cjs') result = sandbox.module.exports; assert.equal(result.value, 37); } assert.equal(sandbox.__factoryRuns, 1); }"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        child
        .stdin
        .take()
        .unwrap()
        .write_all(
          serde_json::json!({ "code": chunk.code, "umd": matches!(format, OutputFormat::Umd), "exported": exported })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let execution = child.wait_with_output().unwrap();
        assert!(
          execution.status.success(),
          "{format:?}/{mode}: {}",
          String::from_utf8_lossy(&execution.stderr)
        );
      }
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn iife_globals_preserve_argument_list_syntax() {
  for global in [
    "(globalThis.__argEvents.push('first'), Dependencies.wrapped), (globalThis.__argEvents.push('extra'), 99)",
    "...[Dependencies.wrapped, (globalThis.__argEvents.push('extra'), 99)]",
    "Dependencies.wrapped // actual user comment\n",
  ] {
    for mode in ["dce", "full"] {
      let calls = Arc::new(AtomicUsize::new(0));
      let callback_calls = Arc::clone(&calls);
      let output = Bundler::new(BundlerOptions {
        cwd: Some(FIXTURE.into()),
        input: Some(vec![InputItem { name: Some("main".into()), import: "./named.js".into() }]),
        external: Some(vec!["wrapped-external".into()].into()),
        globals: Some(GlobalsOutputOption::Fn(Arc::new(move |_| {
          callback_calls.fetch_add(1, Ordering::Relaxed);
          Box::pin(async move { Ok(global.into()) })
        }))),
        format: Some(OutputFormat::Iife),
        name: Some("Wrapped".into()),
        minify: Some(minify(mode)),
        sourcemap: Some(SourceMapType::File),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      assert_eq!(calls.load(Ordering::Relaxed), 1);
      let chunk = output
        .assets
        .iter()
        .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
        .unwrap();
      let json = chunk.map.as_ref().unwrap().to_json_string();
      let map = oxc_sourcemap::SourceMap::from_json_string(&json).unwrap();
      super::map_assertions::assert_valid_map(&chunk.code, &map);
      let mut child = Command::new("node").args(["--input-type=module", "--eval", "import vm from 'node:vm'; import assert from 'node:assert/strict'; let input = ''; for await (const part of process.stdin) input += part; const { code, expected } = JSON.parse(input); const sandbox = { __argEvents: [], __wrappedEvents: [], __wrappedInput: 40, Dependencies: { wrapped: Object.assign(value => value + 2, { offset: 3 }) } }; vm.runInNewContext(code, sandbox); assert.equal(sandbox.Wrapped.read().value, 42); assert.deepEqual(sandbox.__argEvents, expected);"]).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
      let expected: &[&str] = if global.starts_with("...") {
        &["extra"]
      } else if global.contains("'first'") {
        &["first", "extra"]
      } else {
        &[]
      };
      child
        .stdin
        .take()
        .unwrap()
        .write_all(
          serde_json::json!({ "code": chunk.code, "expected": expected }).to_string().as_bytes(),
        )
        .unwrap();
      let execution = child.wait_with_output().unwrap();
      assert!(
        execution.status.success(),
        "{global}/{mode}: {}",
        String::from_utf8_lossy(&execution.stderr)
      );
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn wrappers_preserve_legal_debug_header() {
  for format in [OutputFormat::Iife, OutputFormat::Umd] {
    for legal in [false, true] {
      let output = Bundler::new(BundlerOptions {
        cwd: Some(FIXTURE.into()),
        input: Some(vec![InputItem { name: Some("main".into()), import: "./addons.js".into() }]),
        format: Some(format),
        minify: Some(RawMinifyOptions::Bool(true)),
        comments: Some(CommentsOptions { legal, annotation: false, jsdoc: false }),
        experimental: Some(rolldown_common::ExperimentalOptions {
          attach_debug_info: Some(rolldown_common::AttachDebugInfo::Full),
          ..Default::default()
        }),
        ..Default::default()
      })
      .unwrap()
      .generate()
      .await
      .unwrap();
      let chunk = output
        .assets
        .iter()
        .find_map(|asset| if let Output::Chunk(chunk) = asset { Some(chunk) } else { None })
        .unwrap();
      assert_eq!(chunk.code.contains("//! "), legal, "{format:?}/{legal}: {}", chunk.code);
    }
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn named_exports_and_external_interop() {
  check_wrapped_formats("named", false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn default_exports_and_external_interop() {
  check_wrapped_formats("default", false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_exports_preserve_effects() {
  check_wrapped_formats("none", false).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn extend_preserves_dotted_global_identity() {
  check_wrapped_formats("named", true).await;
}
