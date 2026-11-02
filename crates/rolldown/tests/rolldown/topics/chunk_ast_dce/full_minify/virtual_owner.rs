use std::{borrow::Cow, sync::Arc};

use rolldown::{Bundler, BundlerOptions, InputItem, SourceMapType};
use rolldown_common::Output;
use rolldown_plugin::{
  HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
  HookResolveIdReturn, HookTransformArgs, HookTransformOutput, HookTransformReturn, HookUsage,
  Plugin, PluginContext, SharedLoadPluginContext, SharedTransformPluginContext,
};
use string_wizard::{MagicString, SourceMapOptions};

const FIXTURE: &str =
  concat!(env!("CARGO_MANIFEST_DIR"), "/tests/rolldown/topics/chunk_ast_dce/full_minify");
const VIRTUAL_ENTRY: &str = "\0mapped-owner";

#[derive(Debug)]
struct MappedVirtualEntry {
  map_source: bool,
}

impl Plugin for MappedVirtualEntry {
  fn name(&self) -> Cow<'static, str> {
    "mapped-virtual-entry".into()
  }

  fn register_hook_usage(&self) -> HookUsage {
    HookUsage::ResolveId | HookUsage::Load | HookUsage::Transform
  }

  async fn resolve_id(
    &self,
    _ctx: &PluginContext,
    args: &HookResolveIdArgs<'_>,
  ) -> HookResolveIdReturn {
    let id = match args.specifier {
      "virtual-entry" => VIRTUAL_ENTRY.to_string(),
      "mapped-source" => format!("{FIXTURE}/mapped.js"),
      _ => return Ok(None),
    };
    Ok(Some(HookResolveIdOutput { id: id.into(), ..Default::default() }))
  }

  async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
    if args.id != VIRTUAL_ENTRY {
      return Ok(None);
    }
    Ok(Some(HookLoadOutput {
      code: "import { value } from 'mapped-source'; export const result = { value, virtual: 'virtual-only-marker' };".into(),
      ..Default::default()
    }))
  }

  async fn transform(
    &self,
    _ctx: SharedTransformPluginContext,
    args: &HookTransformArgs<'_>,
  ) -> HookTransformReturn {
    if args.id != VIRTUAL_ENTRY && !(self.map_source && args.id.ends_with("/mapped.js")) {
      return Ok(None);
    }
    let mut code = MagicString::new(args.code.as_str());
    code.prepend("// transform line one\n// transform line two\n");
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

#[tokio::test(flavor = "multi_thread")]
async fn virtual_owner_composes_foreign_source() {
  for (full, map_source) in [(false, false), (false, true), (true, false), (true, true)] {
    let mut bundler = Bundler::with_plugins(
      BundlerOptions {
        input: Some(vec![InputItem { name: Some("main".into()), import: "virtual-entry".into() }]),
        cwd: Some(FIXTURE.into()),
        sourcemap: Some(SourceMapType::File),
        minify: full.then(|| true.into()),
        ..Default::default()
      },
      vec![Plugin::new_shared(MappedVirtualEntry { map_source })],
    )
    .unwrap();
    let output = bundler.generate().await.unwrap();
    let chunk = output
      .assets
      .iter()
      .find_map(|asset| match asset {
        Output::Chunk(chunk) => Some(chunk),
        Output::Asset(_) => None,
      })
      .unwrap();
    let pos = chunk.modules.keys.iter().position(|id| id.as_str() == VIRTUAL_ENTRY).unwrap();
    let owner = &chunk.modules.values[pos];
    assert!(
      owner.code().unwrap().contains("mapped-marker"),
      "regression must exercise code moved into a virtual owner"
    );
    let marker = chunk.code.find("mapped-marker").unwrap();
    let prefix = &chunk.code[..marker];
    let line = u32::try_from(prefix.bytes().filter(|&c| c == b'\n').count()).unwrap();
    let column = u32::try_from(prefix.rsplit('\n').next().unwrap().encode_utf16().count()).unwrap();
    let map = chunk.map.as_ref().unwrap();
    let lookup = map.generate_lookup_table();
    let token = map.lookup_source_view_token(&lookup, line, column).unwrap_or_else(|| {
      panic!("foreign source mapping must survive the owner's chain (full={full}, map_source={map_source})")
    });
    assert_eq!(
      token.get_src_line(),
      0,
      "must use the foreign source's own coordinates (full={full}, map_source={map_source})"
    );
    assert_eq!(
      usize::try_from(token.get_src_col()).unwrap(),
      include_str!("mapped.js").find("'mapped-marker'").unwrap()
    );
    let source = map.get_source(token.get_source_id().unwrap()).unwrap();
    assert!(source.ends_with("mapped.js"), "{source}");
    assert_eq!(
      map.get_source_content(token.get_source_id().unwrap()),
      Some(include_str!("mapped.js"))
    );
    let marker = chunk.code.find("virtual-only-marker").unwrap();
    let prefix = &chunk.code[..marker];
    let line = u32::try_from(prefix.bytes().filter(|&c| c == b'\n').count()).unwrap();
    let column = u32::try_from(prefix.rsplit('\n').next().unwrap().encode_utf16().count()).unwrap();
    let virtual_token = map.lookup_source_view_token(&lookup, line, column);
    assert!(
      virtual_token.is_none_or(|token| token.get_source_id().is_none()),
      "virtual code must not inherit the preceding real source mapping (full={full}, map_source={map_source}): {virtual_token:?}"
    );
  }
}
