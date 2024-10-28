use criterion::{criterion_group, criterion_main, Criterion};
use oxc::{
  allocator::Allocator,
  codegen::{CodeGenerator, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};
use rolldown_sourcemap::{collapse_sourcemaps, ConcatSource, SourceMapSource};
use rolldown_testing::workspace::root_dir;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("remapping");

  let mut sourcemap_chain = vec![];

  let filename = root_dir().join("tmp/bench/antd/antd.js").to_str().unwrap().to_string();
  let source_text = std::fs::read_to_string(&filename).unwrap();
  let source_type = SourceType::from_path(&filename).unwrap();
  let allocator = Allocator::default();
  let ret1 = Parser::new(&allocator, &source_text, source_type).parse();

  let options =
    CodegenOptions { source_map_path: Some(filename.into()), ..CodegenOptions::default() };

  let CodegenReturn { map, code } =
    CodeGenerator::new().with_options(options.clone()).build(&ret1.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  let ret2 = Parser::new(&allocator, &code, source_type).parse();
  let CodegenReturn { map, code: _ } =
    CodeGenerator::new().with_options(options.clone()).build(&ret2.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  group.sample_size(20);
  group.bench_with_input("remapping", &sourcemap_chain, move |b, sourcemap_chain| {
    b.iter(|| {
      let map = collapse_sourcemaps(sourcemap_chain.to_vec());
      map.to_json_string();
    });
  });

  // simulate render-chunk-remapping
  let mut sourcemap_chain = vec![];
  let line = code.matches('\n').count() as u32;
  let mut concat_source = ConcatSource::default();
  let mut sources = vec![];
  for i in 0..3 {
    sources.push(format!("{i}.js"));
    concat_source.add_source(Box::new(SourceMapSource::new(
      code.clone(),
      map.as_ref().unwrap().clone(),
      line,
    )));
  }
  let (source_text, mut source_map) = concat_source.content_and_sourcemap();
  // The sources should be different at common case.
  source_map.as_mut().unwrap().set_sources(sources.iter().map(|s| s.as_str()).collect());
  sourcemap_chain.push(source_map.as_ref().unwrap());

  let ret3 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code: _ } =
    CodeGenerator::new().with_options(options.clone()).build(&ret3.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  group.bench_with_input("render-chunk-remapping", &sourcemap_chain, move |b, sourcemap_chain| {
    b.iter(|| {
      let map = collapse_sourcemaps(sourcemap_chain.to_vec());
      map.to_json_string();
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
