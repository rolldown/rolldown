#[global_allocator]
static ALLOC: mimalloc_safe::MiMalloc = mimalloc_safe::MiMalloc;

use std::fmt::Write as _;

use bench::{BenchMode, DeriveOptions, bench_preset, rome_ts_preset, run_bench_group};
use criterion::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
use oxc::{
  allocator::Allocator,
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};
use rolldown::{BundlerOptions, ModuleType};
use rolldown_sourcemap::{SourceJoiner, SourceMap, SourceMapSource, collapse_sourcemaps};
use rustc_hash::FxHashMap;

fn items() -> Vec<(&'static str, BundlerOptions)> {
  vec![
    ("threejs", bench_preset("threejs", "tmp/bench/three", "entry.js")),
    ("threejs-sourcemap", {
      let mut opts = bench_preset("threejs", "tmp/bench/three", "entry.js");
      opts.sourcemap = Some(rolldown::SourceMapType::File);
      opts
    }),
    ("rome_ts", rome_ts_preset()),
    ("multi-duplicated-top-level-symbol", {
      let mut opts = bench_preset(
        "multi_duplicated_symbol",
        "tmp/bench/rolldown-benchcases/packages/multi-duplicated-symbols",
        "index.jsx",
      );
      opts.module_types = Some(FxHashMap::from_iter([("css".to_string(), ModuleType::Empty)]));
      opts
    }),
  ]
}

fn criterion_benchmark(c: &mut Criterion) {
  let derive_options = DeriveOptions { sourcemap: false, minify: false };
  run_bench_group(c, "bundle", BenchMode::Bundle, &derive_options, items());
}

// ===========================================================================
// rolldown_sourcemap micro-benchmarks
//
// These isolate the per-chunk sourcemap machinery — `SourceJoiner::join`
// (chunk assembly + sourcemap merge) and `collapse_sourcemaps` (the
// minify / transform chains) — which run on the codegen hot path of every
// build, so a regression there surfaces as its own CodSpeed delta instead of
// being diluted inside the end-to-end bundle numbers.
//
// Inputs are REAL sourcemaps produced by oxc codegen (irregular tokens, real
// names, `sourcesContent`, non-identity mappings), generated once outside the
// timed loop — so each case exercises the full machinery on realistic data
// while the measured region stays inside rolldown_sourcemap.
// ===========================================================================

/// A standalone, valid ES module whose body size varies with `index`. A chunk
/// built from many of them gets the irregular token/name distribution of a real
/// app chunk: shared imports (`sharedHelper`/`transform`/`validate`) plus
/// per-module unique identifiers exercise real name/source dedup pressure.
fn module_source(index: u32) -> String {
  let mut steps = String::new();
  for j in 0..=(index % 5) {
    let _ = writeln!(steps, "const step_{j} = items.filter((item) => item.weight > {j}).length;");
  }
  // Indentation is irrelevant — oxc parses this and re-emits its own formatting.
  format!(
    "import {{ sharedHelper, transform, validate }} from \"./shared\";\n\
     export function process_{index}(input, config) {{\n\
     const items = input.values.map((value) => transform(value, config.scale));\n\
     {steps}\
     const total = items.reduce((sum, item) => sum + item.weight, 0);\n\
     if (total > config.threshold) {{\n\
     return {{ ok: true, items, total, label: \"module_{index}\" }};\n\
     }}\n\
     return sharedHelper(validate(items), total);\n\
     }}\n\
     export const META_{index} = {{ id: {index}, name: \"process_{index}\", enabled: true }};\n"
  )
}

/// A single larger module with `funcs` exported functions — the input program
/// for the collapse-chain cases (one big map rather than many small ones).
fn program_source(funcs: u32) -> String {
  let mut body = String::new();
  for i in 0..funcs {
    let _ = writeln!(body, "export function fn_{i}(input) {{");
    let _ = writeln!(body, "const mapped = input.map((value) => transform(value, {i}));");
    let _ = writeln!(body, "return sharedHelper(mapped.filter((item) => item.weight > {i}));");
    let _ = writeln!(body, "}}");
  }
  format!("import {{ sharedHelper, transform }} from \"./shared\";\n{body}")
}

/// Parse `source` and run it through oxc codegen, returning the generated code
/// and its (real) sourcemap. `minify` toggles compact output, so chaining calls
/// produces a realistic non-identity remap for `collapse_sourcemaps`.
fn codegen_map(source: &str, path: &str, minify: bool) -> (String, SourceMap) {
  let allocator = Allocator::default();
  let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
  let CodegenReturn { code, map, .. } = Codegen::new()
    .with_options(CodegenOptions {
      minify,
      source_map_path: Some(path.into()),
      ..CodegenOptions::default()
    })
    .build(&parsed.program);
  (code, map.expect("codegen should emit a source map").into_owned())
}

/// A `SourceJoiner` for a realistic chunk: a `prepend_source` banner,
/// `module_count` modules each carrying a real per-module sourcemap, and a plain
/// `append_source` footer. The banner uses `prepend_source` (the production
/// banner/hashbang path) and the plain banner/footer exercise the plain↔map
/// line-offset tracking `join` does in production.
fn build_chunk_joiner(module_count: u32) -> SourceJoiner<'static> {
  let mut joiner = SourceJoiner::default();
  joiner.prepend_source("/*! bundled with rolldown */\n".to_string());
  for index in 0..module_count {
    let (code, map) = codegen_map(&module_source(index), &format!("src/module_{index}.js"), false);
    // Production (`render_ecma_module.rs`) wraps module sources with
    // `with_pre_compute_sourcemap_data(true)` so `join` reads a cached line
    // count instead of re-scanning each body — mirror it.
    joiner.append_source(SourceMapSource::new(code, map).with_pre_compute_sourcemap_data(true));
  }
  joiner.append_source("\n//# sourceMappingURL=chunk.js.map\n".to_string());
  joiner
}

/// A `SourceJoiner` of plain (map-less) module sources — the `enable_sourcemap`
/// == false fast path (`output.sourcemap` unset, the most common production
/// build), where `join` skips `ConcatSourceMapBuilder` entirely.
fn build_plain_joiner(module_count: u32) -> SourceJoiner<'static> {
  let mut joiner = SourceJoiner::default();
  for index in 0..module_count {
    // No map needed on this path, so skip codegen and feed the raw module text.
    joiner.append_source(module_source(index));
  }
  joiner
}

/// Scale of a large, messy real-world chunk (a "shit-mountain" / 屎山 bundle), so
/// the benches reflect the cost on the kind of target users actually throw at the
/// bundler and a size-dependent regression shows up.
const BIG_CHUNK: u32 = 2000;

fn sourcemap_benches(c: &mut Criterion) {
  let mut group = c.benchmark_group("sourcemap");

  // `join` consumes the joiner, so it's rebuilt per iteration in the (excluded) setup; each measured iteration is one full chunk assembly + sourcemap merge.

  // With sourcemaps: a large (shit-mountain-scale) chunk of mapped modules +
  // banner/footer — drives `ConcatSourceMapBuilder` (token buffer + dedup + line offsets).
  group.bench_function("join_with_sourcemap", |b| {
    b.iter_batched(
      || build_chunk_joiner(BIG_CHUNK),
      |chunk| black_box(chunk.join()),
      BatchSize::SmallInput,
    );
  });

  // Without sourcemaps: the same module bodies as plain sources — the
  // `enable_sourcemap == false` fast path that skips the builder entirely (the
  // most common production build, `output.sourcemap` unset).
  group.bench_function("join_no_sourcemap", |b| {
    b.iter_batched(
      || build_plain_joiner(BIG_CHUNK),
      |chunk| black_box(chunk.join()),
      BatchSize::SmallInput,
    );
  });

  // `collapse_sourcemaps` — the module-transform (`render_chunks.rs`) and
  // post-minification (`minify_chunks.rs`) chains: a real oxc chain
  // (pretty -> minify -> pretty, 3 links) over a large program so the per-token
  // remap loop dominates.
  let program = program_source(BIG_CHUNK);
  let (pretty_code, pretty_map) = codegen_map(&program, "chunk.js", false);
  let (minified_code, minified_map) = codegen_map(&pretty_code, "chunk.js", true);
  let (_repretty_code, repretty_map) = codegen_map(&minified_code, "chunk.js", false);
  group.bench_function("collapse_codegen_chain", |b| {
    b.iter(|| black_box(collapse_sourcemaps(&[&pretty_map, &minified_map, &repretty_map])));
  });

  group.finish();
}

criterion_group!(benches, criterion_benchmark, sourcemap_benches);
criterion_main!(benches);
