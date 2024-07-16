use criterion::{criterion_group, criterion_main, Criterion};
use oxc::{
  allocator::Allocator,
  codegen::{CodeGenerator, CodegenReturn},
  parser::Parser,
  span::SourceType,
};
use rolldown_sourcemap::collapse_sourcemaps;
use rolldown_testing::workspace::root_dir;

fn criterion_benchmark(c: &mut Criterion) {
  let mut group = c.benchmark_group("remapping");

  let mut sourcemap_chain = vec![];

  let filename = root_dir().join("tmp/bench/antd/antd.js").to_str().unwrap().to_string();
  let source_text = std::fs::read_to_string(&filename).unwrap();
  let source_type = SourceType::from_path(&filename).unwrap();
  let allocator = Allocator::default();
  let ret1 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { source_map, source_text } =
    CodeGenerator::new().enable_source_map(&filename, &source_text).build(&ret1.program);
  sourcemap_chain.push(source_map.as_ref().unwrap());

  let ret2 = Parser::new(&allocator, &source_text, source_type).parse();
  let CodegenReturn { source_map, source_text: _ } =
    CodeGenerator::new().enable_source_map(&filename, &source_text).build(&ret2.program);
  sourcemap_chain.push(source_map.as_ref().unwrap());

  group.sample_size(20);
  group.bench_with_input("remapping", &sourcemap_chain, move |b, sourcemap_chain| {
    b.iter(|| {
      let map = collapse_sourcemaps(sourcemap_chain.to_vec()).unwrap();
      map.to_json_string().unwrap();
    });
  });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
