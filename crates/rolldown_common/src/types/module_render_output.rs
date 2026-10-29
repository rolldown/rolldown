use rolldown_sourcemap::SourceMap;

#[derive(Debug)]
pub struct ModuleRenderOutput {
  pub code: String,
  pub map: Option<SourceMap>,
  /// PoC (`chunk_ast_dce` global mode): `map` is already composed with the plugin
  /// `sourcemap_chain`s of every module it maps into, so it must not be collapsed again.
  pub map_is_final: bool,
}
