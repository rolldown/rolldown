// Validation-only borrowed counterpart of the existing mapped-chunk input.
fn borrowed_chunk_sources(module_count: u32) -> Vec<Box<dyn rolldown_sourcemap::Source + Send + Sync>> {
  (0..module_count).map(|index| {
    let (code, map) = codegen_map(&module_source(index), &format!("src/module_{index}.js"), false);
    Box::new(SourceMapSource::new(code, map).with_pre_compute_sourcemap_data(true)) as Box<dyn rolldown_sourcemap::Source + Send + Sync>
  }).collect()
}

fn borrowed_chunk_joiner(sources: &[Box<dyn rolldown_sourcemap::Source + Send + Sync>]) -> SourceJoiner<'_> {
  let mut joiner = SourceJoiner::default();
  joiner.prepend_source("/*! bundled with rolldown */\n".to_string());
  for source in sources { joiner.append_source(source); }
  joiner.append_source("\n//# sourceMappingURL=chunk.js.map\n".to_string());
  joiner
}
