use criterion::{Criterion, criterion_group, criterion_main};
use oxc::{
  allocator::Allocator,
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};
use rolldown_sourcemap::{SourceJoiner, SourceMapSource, collapse_sourcemaps};
use rolldown_workspace::root_dir;

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

  let CodegenReturn { map, code, .. } =
    Codegen::new().with_options(options.clone()).build(&ret1.program);
  sourcemap_chain.push(map.as_ref().unwrap());

  let ret2 = Parser::new(&allocator, &code, source_type).parse();
  let CodegenReturn { map, code: _, .. } =
    Codegen::new().with_options(options.clone()).build(&ret2.program);
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
  let mut source_joiner = SourceJoiner::default();
  let mut sources = vec![];
  for i in 0..3 {
    sources.push(format!("{i}.js"));
    source_joiner.append_source(
      SourceMapSource::new(code.clone(), map.as_ref().unwrap().clone())
        .with_pre_compute_sourcemap_data(true),
    );
  }
  let (source_text, mut source_map) = source_joiner.join();
  // The sources should be different at common case.
  source_map.as_mut().unwrap().set_sources(sources.iter().map(|s| s.as_str()).collect());
  sourcemap_chain.push(source_map.as_ref().unwrap());

  let ret3 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { map, code: _, .. } =
    Codegen::new().with_options(options.clone()).build(&ret3.program);
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
