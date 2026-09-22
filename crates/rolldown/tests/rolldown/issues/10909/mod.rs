use std::borrow::Cow;

use rolldown::{BundlerOptions, InputItem};
use rolldown_common::{Output, ResolveOptions};
use rolldown_plugin::{
  __inner::SharedPluginable, HookResolveIdArgs, HookResolveIdOutput, HookResolveIdReturn,
  HookUsage, Plugin, PluginContext,
};
use rolldown_testing::{manual_integration_test, test_config::TestMeta};

const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/issues/10909");

/// Resolves the aliases the way `vite-tsconfig-paths` does: it asks `ctx.resolve` for the real
/// file, then returns only the id string. That is a supported `resolveId` return type, so the
/// resulting module must still get its `package.json#sideEffects` policy.
#[derive(Debug)]
struct IdStringAlias;

impl Plugin for IdStringAlias {
  fn name(&self) -> Cow<'static, str> {
    "id-string-alias".into()
  }

  async fn resolve_id(
    &self,
    ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let target = match args.specifier {
      "@unused" => "./unused.js",
      // The two `dep` modules pin where the walk must stop. `dep/lib` carries its own manifest,
      // and the fixture root declares `sideEffects: false`. Only a stop at `dep`'s own manifest
      // drops one module and keeps the other. See the test below.
      "@nested" => "dep/lib/nested.js",
      "@effectful" => "dep/lib/effectful.js",
      "@fmt" => "fmt/lib/importer.js",
      "@fmt-jsx" => "fmt/lib/importer.jsx",
      // Returned as a ready absolute path. `ctx.resolve` would already apply `resolve.alias` to
      // the resolved file, and the point is a hook whose own answer differs from the alias.
      "@alias-src" => {
        return Ok(Some(HookResolveIdOutput::from_id(format!("{FIXTURE_DIR}/alias-src/index.js"))));
      }
      _ => return Ok(None),
    };
    let resolved =
      ctx.resolve(target, args.importer, None).await?.expect("the target should resolve");
    Ok(Some(HookResolveIdOutput::from_id(resolved.id.as_arc_str().clone())))
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId
  }
}

/// Bundles `entry` with the alias plugin and joins every emitted chunk.
async fn bundle(entry: &str) -> String {
  bundle_with(entry, vec![Plugin::new_shared(IdStringAlias)], None).await
}

async fn bundle_with(
  entry: &str,
  plugins: Vec<SharedPluginable>,
  resolve: Option<ResolveOptions>,
) -> String {
  let output = manual_integration_test!()
    .build(TestMeta {
      snapshot: false,
      write_to_disk: false,
      expect_executed: false,
      ..Default::default()
    })
    .bundle_with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("entry".to_string()), import: entry.to_string() }]),
        resolve,
        ..Default::default()
      },
      plugins,
    )
    .await
    .expect("Failed to bundle");

  output
    .assets
    .iter()
    .filter_map(|asset| match asset {
      Output::Chunk(chunk) => Some(chunk.code.as_str()),
      Output::Asset(_) => None,
    })
    .collect()
}

/// A module's side-effect policy must not depend on the specifier that reached it. In #10909 it
/// broke that. The module loader picked the winning resolution in task-completion order, so the
/// same input produced two different bundles. One importer is enough to pin the invariant.
#[tokio::test(flavor = "multi_thread")]
async fn plugin_resolved_id_keeps_package_side_effects_policy() {
  let code = bundle("./entry.js").await;

  for marker in ["THIS SHOULD BE TREE-SHAKEN", "NESTED SHOULD BE TREE-SHAKEN"] {
    assert!(
      !code.contains(marker),
      "`{marker}` sits in a package whose `sideEffects` excludes it, so it must be shaken out \
       even though a plugin resolved it to a bare id string. Output:\n{code}"
    );
  }

  // `dep` lists this file in its own `sideEffects`, while the fixture root above `node_modules`
  // declares `sideEffects: false`. A walk that ran past the `node_modules` boundary would take
  // the root's answer and drop this module.
  assert!(
    code.contains("EFFECTFUL MUST SURVIVE"),
    "`dep` declares `lib/effectful.js` side-effectful, so it must survive. Output:\n{code}"
  );
}

/// The same invariant for the module format. Node.js reads `"type"` from a different manifest
/// than the one that owns `sideEffects`. `fmt` declares `"type": "module"` and `fmt/lib`
/// declares `"type": "commonjs"`, so each route must still agree on the importer's interop.
///
/// The two extensions take different branches. `oxc_resolver` reads `"type"` from the nearest
/// scope for `.js`, and reads no type for `.jsx`. Rolldown then falls back to the owning
/// manifest. One manifest for both would invert one of them. An aliased importer would use
/// `__toESM(mod, 1)` where direct resolution uses `__toESM(mod)`. `import v from 'cjs'` would
/// yield `{ __esModule: true, default: 123 }` instead of `123`.
#[tokio::test(flavor = "multi_thread")]
async fn plugin_resolved_id_keeps_package_type_precedence() {
  for (direct_entry, aliased_entry) in [
    ("./direct-format.js", "./aliased-format.js"),
    ("./direct-jsx-format.js", "./aliased-jsx-format.js"),
  ] {
    let direct =
      bundle(direct_entry).await.replace(direct_entry.trim_start_matches("./"), "<entry>");
    let aliased =
      bundle(aliased_entry).await.replace(aliased_entry.trim_start_matches("./"), "<entry>");

    assert_eq!(
      direct, aliased,
      "resolving `{direct_entry}`'s importer through a plugin must not change its interop"
    );
  }
}

/// A `resolveId` hook's id is final. Recovering its manifests must read that exact file, not
/// what `resolve.alias` or another rewrite would turn the path into. Here an alias maps
/// `alias-src/index.js` to `alias-target/index.js`, whose package declares `sideEffects: false`.
/// Reading the target's manifest would drop the source module's side effect.
#[tokio::test(flavor = "multi_thread")]
async fn plugin_resolved_id_reads_metadata_from_the_returned_path() {
  let resolve = ResolveOptions {
    alias: Some(vec![(
      format!("{FIXTURE_DIR}/alias-src/index.js"),
      vec![Some(format!("{FIXTURE_DIR}/alias-target/index.js"))],
    )]),
    ..Default::default()
  };
  let code =
    bundle_with("./alias-entry.js", vec![Plugin::new_shared(IdStringAlias)], Some(resolve)).await;

  assert!(
    code.contains("ALIAS SOURCE MUST SURVIVE"),
    "`alias-src` declares `sideEffects: true`, so its module must survive even though an alias \
     maps its path elsewhere. Output:\n{code}"
  );
}
