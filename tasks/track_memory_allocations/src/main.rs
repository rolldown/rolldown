//! Tracks the number of heap allocations made by `rolldown_sourcemap`'s
//! per-chunk machinery (`SourceJoiner::join` and `collapse_sourcemaps`) for a
//! fixed set of scenarios, and writes the counts to a committed snapshot
//! (`allocs.snap`).
//!
//! CI re-runs this and fails if the snapshot changes, so any allocation
//! regression (or improvement) in these components surfaces as a reviewable
//! diff. We track the *number* of allocations rather than bytes: the count is
//! stable across platforms and closely tracks memory pressure, whereas byte
//! totals vary between allocators/platforms (same rationale as oxc's tool).

use std::{
  alloc::{GlobalAlloc, Layout},
  fmt::Write as _,
  hint::black_box,
  sync::atomic::{AtomicUsize, Ordering::SeqCst},
};

use mimalloc_safe::MiMalloc;
use oxc::{
  allocator::Allocator,
  codegen::{Codegen, CodegenOptions, CodegenReturn},
  parser::Parser,
  span::SourceType,
};
use rolldown_sourcemap::{SourceJoiner, SourceMap, SourceMapSource, collapse_sourcemaps};

// Count allocations through a fixed allocator (MiMalloc) so the numbers don't
// depend on the platform's default system allocator.
#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

static NUM_ALLOC: AtomicUsize = AtomicUsize::new(0);
static NUM_REALLOC: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

// SAFETY: every method delegates to `MiMalloc`, which is a sound `GlobalAlloc`.
unsafe impl GlobalAlloc for CountingAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let ptr = unsafe { MiMalloc.alloc(layout) };
    if !ptr.is_null() {
      NUM_ALLOC.fetch_add(1, SeqCst);
    }
    ptr
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let ptr = unsafe { MiMalloc.alloc_zeroed(layout) };
    if !ptr.is_null() {
      NUM_ALLOC.fetch_add(1, SeqCst);
    }
    ptr
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    let new_ptr = unsafe { MiMalloc.realloc(ptr, layout, new_size) };
    if !new_ptr.is_null() {
      NUM_REALLOC.fetch_add(1, SeqCst);
    }
    new_ptr
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    // SAFETY: same contract, forwarded to `MiMalloc`.
    unsafe { MiMalloc.dealloc(ptr, layout) };
  }
}

struct Row {
  label: String,
  allocs: usize,
  reallocs: usize,
}

/// Reset the counters, run `op`, and record the allocations it makes. Inputs are
/// built by the caller *before* this is called, so the measured window is just
/// the `rolldown_sourcemap` operation (`join` / `collapse_sourcemaps`).
fn measure<R>(rows: &mut Vec<Row>, label: &str, op: impl FnOnce() -> R) {
  NUM_ALLOC.store(0, SeqCst);
  NUM_REALLOC.store(0, SeqCst);
  let result = op();
  let allocs = NUM_ALLOC.load(SeqCst);
  let reallocs = NUM_REALLOC.load(SeqCst);
  black_box(&result);
  rows.push(Row { label: label.to_owned(), allocs, reallocs });
}

/// A standalone, valid ES module whose body size varies with `index` — a chunk
/// built from many gets the irregular token/source distribution of a real app
/// chunk (shared imports + per-module unique identifiers).
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
/// for the collapse-chain scenarios.
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

/// A `SourceJoiner` for a realistic chunk: a `prepend_source` banner (the
/// production banner/hashbang path), `module_count` modules each carrying a real
/// per-module sourcemap, and a plain `append_source` footer.
fn build_chunk_joiner(module_count: u32) -> SourceJoiner<'static> {
  let mut joiner = SourceJoiner::default();
  joiner.prepend_source("/*! bundled with rolldown */\n".to_string());
  for index in 0..module_count {
    let (code, map) = codegen_map(&module_source(index), &format!("src/module_{index}.js"), false);
    // Mirror production (`render_ecma_module.rs`): pre-compute the line count.
    joiner.append_source(SourceMapSource::new(code, map).with_pre_compute_sourcemap_data(true));
  }
  joiner.append_source("\n//# sourceMappingURL=chunk.js.map\n".to_string());
  joiner
}

/// A `SourceJoiner` of plain (map-less) module sources — the `enable_sourcemap`
/// == false fast path where `join` skips `ConcatSourceMapBuilder` entirely.
fn build_plain_joiner(module_count: u32) -> SourceJoiner<'static> {
  let mut joiner = SourceJoiner::default();
  for index in 0..module_count {
    // No map needed here (this is the no-sourcemap path), so skip codegen and
    // feed the raw module text — keeps the large-scale setup cheap.
    joiner.append_source(module_source(index));
  }
  joiner
}

/// Scale of a large, messy real-world chunk (a "shit-mountain" / 屎山 bundle): a
/// regression that only bites at size — superlinear allocation, or a capacity
/// pre-size that quietly stops holding (→ reallocs appear) — must show up here,
/// so the scenarios are sized to a big chunk, not a toy one.
const BIG_CHUNK: u32 = 2000;

fn collect_rows() -> Vec<Row> {
  let mut rows = Vec::new();

  // `SourceJoiner::join` WITH sourcemaps — per-chunk assembly + merge (the
  // `ConcatSourceMapBuilder` path).
  let mut app_chunk = build_chunk_joiner(BIG_CHUNK);
  measure(&mut rows, "sourcemap/join_with_sourcemap", || app_chunk.join());

  // `join` WITHOUT sourcemaps — the `enable_sourcemap == false` fast path that
  // skips the builder (the most common production build, `output.sourcemap` unset).
  let mut plain_chunk = build_plain_joiner(BIG_CHUNK);
  measure(&mut rows, "sourcemap/join_no_sourcemap", || plain_chunk.join());

  // `collapse_sourcemaps`: real oxc chain, fine-grained, 3 links over a big
  // program → tens of thousands of tokens so the remap loop dominates.
  let program = program_source(BIG_CHUNK);
  let (pretty_code, pretty_map) = codegen_map(&program, "chunk.js", false);
  let (minified_code, minified_map) = codegen_map(&pretty_code, "chunk.js", true);
  let (_repretty_code, repretty_map) = codegen_map(&minified_code, "chunk.js", false);
  measure(&mut rows, "sourcemap/collapse_codegen_chain", || {
    collapse_sourcemaps(&[&pretty_map, &minified_map, &repretty_map])
  });

  rows
}

fn main() {
  // Each scenario resets the counters in `measure`, so no warm-up pass is
  // needed — every measured window starts from zero regardless of order.
  let rows = collect_rows();

  let mut out = String::new();
  let _ = writeln!(out, "{:<32} | {:>10} | {:>10}", "Scenario", "Allocs", "Reallocs");
  out.push_str(&"-".repeat(32 + 10 + 10 + 6));
  out.push('\n');
  for row in &rows {
    let _ = writeln!(out, "{:<32} | {:>10} | {:>10}", row.label, row.allocs, row.reallocs);
  }

  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/allocs.snap");
  std::fs::write(path, out).expect("failed to write allocs snapshot");
}
