# PoC: chunk-level DCE on the module ASTs instead of a chunk re-parse

Branch `poc/chunk-ast-minify`. Not meant to be merged. The question: how much time do we save if
the chunk-level `dce-only` pass runs on an AST assembled from the finalized module ASTs, instead
of printing every module, joining the text and re-parsing the whole chunk in `minify_chunks`?

**Short answer:** with sourcemaps on, the pass drops from 675 → 289 ms CPU on three.js ×10
(−57%), 83 → 41 ms on rome, 170 → 50 ms on antd. Because the pass is serial on a single big
chunk, wall time follows: three.js ×10 builds go from ~1.45 s to ~0.76 s (minify off: ~0.41 s).
Peak RSS does not rise; it drops slightly. Output is semantically equivalent on everything
checked.

Two variants:

- **Barrier mode** (`=1`, §1): no oxc changes, fastest, but output up to 1.8% larger, because a
  module barrier blocks oxc's single-use inlining across module boundaries.
- **Global mode** (`=global`, §1b): spans of all modules are rebased into one per-chunk source
  space, so oxc may move code across modules and every moved node still maps to its own module.
  Output matches main within 0.03% (only kept header/license comments and blank lines differ),
  sourcemaps stay correct. Costs ~65 ms more than barrier mode on three.js ×10 (still ~60% less
  pass CPU than main) and needs a small `oxc_codegen` patch (shareable line tables).

Everything is behind an env var, so main's behavior and the PoC are compared from one build:

| Env var                                   | Effect                                                              |
| ----------------------------------------- | ------------------------------------------------------------------- |
| `ROLLDOWN_CHUNK_AST_DCE=1`                | fast path, with module barrier (correct sourcemaps)                 |
| `ROLLDOWN_CHUNK_AST_DCE=global`           | fast path, no barrier, rebased spans (correct sourcemaps, §1b)      |
| `ROLLDOWN_CHUNK_AST_DCE=nobarrier`        | no barrier when sourcemaps are off (measurement only, see §4)       |
| `ROLLDOWN_CHUNK_AST_DCE=nobarrier-unsafe` | no barrier even with sourcemaps (measurement only, maps get broken) |
| `ROLLDOWN_POC_PROF=1`                     | per-phase thread-CPU accounting, printed by `minify_prof`           |
| `ROLLDOWN_CHUNK_AST_DCE_DEBUG=1`          | eligibility / iteration-count logging                               |
| `ROLLDOWN_CHUNK_AST_DCE_DUMP=<dir>`       | write the assembled chunk program (printed) before DCE              |

With the env var unset, output is byte-identical to main (checked on three.js ×10, rome, antd,
and the full `cargo test -p rolldown` snapshot suite).

## 1. Design: codegen and sourcemaps

**Chosen: "DCE on the assembled AST, then print each module with its own context".** Barrier
mode needs no oxc changes; global mode (§1b) adds a small `oxc_codegen` patch.

In `create_chunk_to_codegen_ret_map` (before modules are printed), for each eligible chunk
(`crates/rolldown/src/stages/generate_stage/chunk_ast_dce.rs`):

1. Every module's top-level statements are moved (`take_in`, no copy) into one chunk `Program`
   whose body `Vec` lives in a fresh per-chunk allocator. Nodes stay in their module arenas;
   lifetimes are erased inside a private chunk owner (intact donor ProgramCells and a boxed
   scratch allocator stay at stable addresses through printing and unwind).
2. After each module an `export {}` marker with a sentinel span (`u32::MAX`) is pushed.
3. The text rolldown would generate for the chunk's imports (`render_esm_chunk_imports`) is
   parsed and put at the head; the wrapped-entry `export default require_x();` and the exports
   (`render_chunk_exports`) are parsed and put at the tail. So exports are DCE roots and imported
   bindings resolve, exactly like in the re-parsed text.
4. `Minifier::dce` runs once on the chunk program (one `SemanticBuilder` + `Stats` + compressor).
5. The body is split back: statements before the first marker that are module declarations =
   head; between markers = one segment per module; after the last marker = tail. Each segment
   becomes that module's `program.body` again. Head and tail are printed with oxc codegen (they
   replace the import/export text in `render_esm`).
6. Modules are then printed exactly as today (`NormalModule::render_with`, each with its own
   source text, comments and sourcemap; annotation comments follow `comments.annotation` instead
   of being forced on), joined by `SourceJoiner`, and `minify_chunks` skips the chunk.

So this removes: the chunk parse, one full codegen of the chunk (+ its sourcemap), and
`collapse_sourcemaps`. It keeps: semantic + DCE (serial, same as today) and the per-module
prints (parallel, already done today).

**Why the marker (barrier).** oxc's DCE-only mode does not reorder top-level statements and has
`sequences`/`join_vars` off, but it does inline single-use variables into the _next_ statement
(`substitute_single_use_symbol_in_statement`). Across a module boundary that moves an expression
from module A into module B. With per-module printing, the moved nodes keep A's spans but are
printed with B's source text, line table and comments: sourcemap positions are wrong (1,026
out-of-bounds mappings on rome with `nobarrier-unsafe`) and `/* @__PURE__ */` annotations of the
moved nodes are lost (oxc codegen prints them from the comment table by span). An `export {}`
marker is declarative (it does not mark the body "unsafe" for hoisted-var inlining), has no
`from` (so it does not trigger the "module has loaders" gate), no specifiers (so nothing merges
into it), and oxc keeps it, so it splits the body exactly and stops cross-module inlining.

**Not chosen: one codegen pass over the whole chunk.**
That would allow cross-module inlining with correct maps. It needs an oxc codegen change: either
codegen that switches source text/comments/line tables per span, or rebasing all spans into one
virtual space (an extra full-AST visitor) plus a new token→module remap with the module
`sourcemap_chain`s. It also makes the final print serial (one codegen of the whole chunk instead
of N parallel module prints), so it would be slower than the chosen design on single-chunk builds.
Global mode (§1b) takes the rebasing idea but keeps the parallel per-module prints.

## 1b. Size parity: global mode (`ROLLDOWN_CHUNK_AST_DCE=global`)

`crates/rolldown/src/stages/generate_stage/chunk_ast_dce/global.rs`. The barrier exists only
because a node oxc moves from module A into module B keeps spans into A's source but is printed
with B's source text, line tables and comment table. Global mode makes spans unambiguous instead:

1. Per chunk, one text `V` = every module's source (each starting on its own line), then the
   generated head (imports) and tail (wrapped entry + exports) text. Every module's spans and
   comments are rebased by the module's offset in `V` (a `VisitMut` over each module AST, in
   parallel across modules), and `program.source_text` becomes `V`. Comments with
   `attached_to == 0` keep it: that is the finalizer's "print at the top of the module" marker
   for comments whose anchor it removed.
2. Module statements are assembled **without markers**; head and tail are parsed and rebased into
   their ranges of `V`. `Minifier::dce` is free to inline across modules, as on the re-parsed
   text.
3. Each surviving module statement goes to the module whose range of `V` contains its span start
   (nodes the finalizer synthesized with `SPAN` were rebased to their module's base, so they still
   name their module; a statement the minifier creates with `SPAN` stays with the previous
   module). Generated statements go to head or tail by **position** (before or after the first
   module statement), not span: oxc merges `import { t } from "./x"` + `export { t }` into an
   `export … from "./x"` that keeps the export's span but the import's position, and the position
   decides when `./x` is evaluated (`code_splitting/chunk_import_source_order`).
4. Each module is printed as today, in parallel, but against `V`, with `V`'s line tables built
   once and shared. That needs the `oxc_codegen` patch below; without it every print would rebuild
   line tables over all of `V` and reserve `|V|` bytes.
5. Each printed map (single source `V`) is remapped token by token to `(module, line, column)`
   using the modules' start lines in `V` (the module's own tokens take a fast path; only foreign
   ones are binary-searched). A module's map can now name other modules as sources (code moved
   into it). Tokens into generated text are dropped. If a map has foreign tokens and any of its
   source modules has a plugin `sourcemap_chain`, the tokens are split per source module, each
   group goes through that module's chain with the existing `collapse_module_sourcemap`, and the
   groups are merged back in generated order; the map is then marked final
   (`ModuleRenderOutput::map_is_final`) so `render_ecma_module` does not collapse it again (it
   still reports `SOURCEMAP_BROKEN` for omitted plugin maps). Without foreign tokens the owner's
   map is composed with its chain exactly as today. Fixture:
   `crates/rolldown/tests/rolldown/topics/chunk_ast_dce/plugin_sourcemap_chain` (a plugin shifts
   every module by two lines with an exact map; oxc inlines `lib.js`'s object into `entry.js`;
   in global mode the moved code maps to the same `lib.js` lines as on main). Head and tail are printed the same
   way (with the user's comment options; they can hold moved code, e.g. `var bar =
   require_foo().bar` gets `require_foo`'s initializer inlined) and joined as `SourceMapSource`s.
   The joined chunk map is de-duplicated by source afterwards.

The `oxc_codegen` patch (vendored as `vendor/oxc_codegen`, wired with `[patch.crates-io]` in the
root `Cargo.toml`; full diff in `vendor/oxc_codegen.patch`, ~140 lines):

- `LineOffsetTables` is public, with `LineOffsetTables::new(source_text)` and
  `line_index_of(byte_offset)`.
- `LineOffsetTables::concat(parts)`: tables of a concatenated text from the parts' tables, so the
  per-module tables are built in parallel (inside the rebase job) instead of one serial scan of
  `V`.
- `SourcemapBuilder` holds its tables in an `Arc` and gains `new_with_line_offset_tables`; the
  token-capacity estimate is capped at 1 Mi (it was `source_len / 4`).
- `Codegen::with_line_offset_tables(Arc<LineOffsetTables>)`: use those tables and skip
  `code.reserve(program.source_text.len())`.

Two bugs came up while getting global mode right, both in the PoC itself: rebasing the
`attached_to == 0` marker lost the comments of removed imports (licence headers), and treating
empty spans as "unknown module" sent the finalizer's CJS wrapper of antd to the runtime module
(which lost all of antd's comments).

**Rolldown bug found and fixed (output-neutral on main).** The finalizer rewrites a top-level
`class C {…}` that references itself into `var C = class C {…}`
(`get_transformed_class_decl`, `crates/rolldown/src/module_finalizers/mod.rs`) but reused the
`Class` box with `r#type == ClassDeclaration`. Text re-parsing never noticed. A semantic pass on
the AST declared `C` a second time in the root scope, which made oxc think `C` is written and
blocked inlining around every `new C(...)`. The fix sets `class.r#type = ClassExpression`. Main's
output is unchanged (all snapshots identical with the flag off).

## 2. What the fast path covers, what falls back

Covered: `minify: 'dce-only'` (the default), ESM output, with or without sourcemaps; chunks
whose text is module code + generated imports/exports + `export default require_x();` of a
CJS-wrapped entry + `export * from 'external'` + hashbang/directives/comment-only banner/footer.
All three benchmarks take it; most of the test suite does.

Falls back to today's path (`chunk_ast_dce::build_eligible` / `chunk_eligible`):

- full minify (`minify: true` or any `mangle`/`mangle_properties`) — mangling and `sequences`
  /`join_vars` would cross modules;
- non-ESM formats (cjs/iife/umd wrappers are text);
- `renderChunk` plugins, except the actual `AssetModulePlugin`, `CopyModulePlugin` and the
  testing-only `HiddenModuleMarkersPlugin` types. The gate checks the underlying concrete type,
  so reusing a builtin name does not bypass fallback. Chunks with emitted-file placeholders
  still fall back to preserve replacement-before-minify ordering;
- `intro`/`outro`, banner/footer functions or plugin hooks, or a static banner/footer that is
  not comment-only (the re-parse also syntax-checks and DCEs them: `errors/chunk_parse_error`);
- dev mode, `experimentalInlineCommonChunks` records/carriers;
- chunks with concatenated wrapped module groups (hoisted vars/fns and `__esm` wrappers are text),
  wrapped entries other than the CJS `export default require_x()` shape, any module with
  `ecma_view.mutations` (MagicString edits), any module body still containing module
  declarations, `.d.ts` chunks.

Behavior differences on the fast path worth knowing:

- `renderChunk` (allowlisted plugins) and `RenderedChunk.modules[id].code` see post-DCE code.
- Generated text that the re-parse used to re-format is now passed through verbatim:
  `export * from "x"` without `;`, directive quote style.

## 3. Numbers

Machine: 4 vCPU cloud VM, `cargo build --profile profile`, mimalloc, `minify_prof` example
(`crates/bench/examples/minify_prof.rs`). Four interleaved rounds of 5 builds per configuration;
per-build medians. "Pass CPU" = in-process thread CPU of the chunk pass: main = parse + DCE +
codegen + collapse in `minify_chunks`; PoC = assemble + DCE + split/print head/tail. Process CPU
= `CLOCK_PROCESS_CPUTIME_ID`. Wall clock on these VMs is noisy (±5–25%, occasional 2×
outliers; ranges are min–max over rounds).

### Sourcemaps on

| bench        | config     | pass CPU (ms) | of which DCE | process CPU (ms) | wall (ms, range)  |
| ------------ | ---------- | ------------: | -----------: | ---------------: | ----------------- |
| three.js ×10 | main       |         674.7 |        298.2 |             2925 | 1451 (1358–1603)  |
| three.js ×10 | **PoC**    |     **288.8** |        287.9 |         **1581** | **755** (697–801) |
| three.js ×10 | minify off |             – |            – |             1186 | 408 (395–465)     |
| rome         | main       |          82.9 |         38.3 |              445 | 229 (178–273)     |
| rome         | **PoC**    |      **40.7** |         40.6 |          **384** | **168** (135–358) |
| rome         | minify off |             – |            – |              356 | 133 (114–157)     |
| antd         | main       |         169.7 |         77.2 |              408 | 413 (396–532)     |
| antd         | **PoC**    |      **49.7** |         49.6 |          **305** | **310** (293–390) |
| antd         | minify off |             – |            – |              306 | 308 (243–321)     |

Main's split on three.js ×10 (maps on): parse 152, DCE 298, codegen+map 174, collapse 51 ms.

### Sourcemaps off (`NO_MAP=1`)

| bench        | config     | pass CPU (ms) | of which DCE | process CPU (ms) | wall (ms, range)  |
| ------------ | ---------- | ------------: | -----------: | ---------------: | ----------------- |
| three.js ×10 | main       |         583.5 |        351.3 |             1719 | 969 (893–1027)    |
| three.js ×10 | **PoC**    |     **321.0** |        320.1 |         **1590** | **747** (614–924) |
| three.js ×10 | minify off |             – |            – |             1081 | 359 (344–374)     |
| rome         | main       |          57.0 |         34.2 |              361 | 174 (152–238)     |
| rome         | **PoC**    |      **49.1** |         48.9 |          473 (*) | 200 (134–312) (*) |
| rome         | minify off |             – |            – |              332 | 125 (98–154)      |
| antd         | main       |         123.1 |         75.8 |              345 | 346 (320–514)     |
| antd         | **PoC**    |      **46.2** |         46.2 |          **295** | **295** (242–421) |
| antd         | minify off |             – |            – |              235 | 237 (204–258)     |

(*) one outlier round (3.4 s CPU for 5 builds vs ~1.6 s in the others); the pass CPU is the
reliable number here.

Reading the numbers:

- The savings are the parse + chunk codegen (+ map) + collapse, as expected: 50–70% of the pass
  with maps, 15–60% without. What remains is semantic + DCE itself, which is still serial.
- On three.js ×10 the remaining DCE on the AST costs about the same as DCE on the parsed text
  (288 vs 298 ms with maps; 320 vs 351 without). On rome it is ~10–40% _slower_ (41 vs 38 ms, 49
  vs 34 ms without maps) with the same iteration count; I did not find the cause. My guess is
  memory locality: nodes spread over ~1,000 module arenas instead of one fresh, sequentially
  allocated parse arena. On antd (one module) it is faster (50 vs 77 ms).
- The per-module prints (already parallel today) cost the same in both modes (three.js ×10 with
  maps: 181 vs 127 ms summed thread CPU; the PoC prints the DCE'd, smaller ASTs).

### Global mode vs barrier mode (second benchmark run)

A second run of the same harness with all four configurations interleaved (`bench2`). This run
was noisier for main (its three.js ×10 pass came out at 968 ms vs 675 ms in the first run), so
compare within the table, not across runs. Per-build medians, sourcemaps on:

| bench        | config     | pass CPU (ms) |   DCE | rebase + space (ms) | module prints (ms, Σ threads) | wall (ms, range)   |
| ------------ | ---------- | ------------: | ----: | ------------------: | ----------------------------: | ------------------ |
| three.js ×10 | main       |         967.6 | 384.8 |                   – |                         209.8 | 1699 (1559–3669)   |
| three.js ×10 | barrier    |         321.8 | 320.9 |                   – |                         134.3 | 800 (740–922)      |
| three.js ×10 | **global** |     **386.6** | 326.9 |           34.4+24.2 |                         221.0 | **985** (769–1036) |
| three.js ×10 | minify off |             – |     – |                   – |                         133.7 | 507 (452–560)      |
| rome         | main       |          70.9 |  34.9 |                   – |                          24.3 | 189 (176–202)      |
| rome         | barrier    |          37.6 |  37.5 |                   – |                          22.4 | 154 (145–187)      |
| rome         | **global** |      **49.6** |  38.8 |             5.5+5.1 |                          25.0 | **163** (159–180)  |
| antd         | main       |         154.1 |  77.6 |                   – |                          33.1 | 406 (389–455)      |
| antd         | barrier    |          47.6 |  47.5 |                   – |                          30.0 | 296 (289–361)      |
| antd         | **global** |      **56.2** |  44.9 |             5.2+6.2 |                          49.9 | **356** (301–426)  |

Sourcemaps off, pass CPU: three.js ×10 main 577.6 / barrier 376.7 / global 437.5; rome 55.0 /
38.7 / 46.5; antd 123.8 / 47.3 / 53.2.

What global mode costs on top of barrier mode:

- "space" (concatenating the module sources into `V` and building its line tables once,
  serial): 24–36 ms on three.js ×10, ~5 ms on rome/antd.
- "rebase" (one `VisitMut` over every module AST, parallel; summed thread CPU): ~35 ms on three.js
  ×10, ~5 ms on rome/antd.
- the module prints get more expensive (three.js ×10: 221 vs 134 ms summed thread CPU) because of
  the token remap from `V` to module coordinates and because each module map copies its own
  source content again. That work is parallel; with sourcemaps off the prints cost the same as in
  barrier mode.

After this run, two cheap optimizations went in (outputs byte-identical): the remap skips the
module search for the printed module's own tokens (remap ≈ 45 → 23 ms thread CPU per three.js
×10 build), and the line tables are built per module in the parallel rebase job and concatenated
(the serial "space" step is ~14 ms per warm build; the larger averages above include a cold
first iteration of ~130 ms). A short re-measure afterwards (three.js ×10,
sourcemaps on, 4 interleaved rounds of 5 builds; the VM was very noisy, DCE alone varied 1.7–2.3 s
per 5 builds) gave per-build median walls of ~1.55 s (main), ~1.08 s (barrier) and ~1.10 s
(global): global mode is within noise of barrier mode. Its extra thread CPU per build: rebase ~62
ms (parallel), space ~30–48 ms (incl. one cold iteration per run), remap ~27 ms (parallel).

- DCE costs the same (the barrier markers were not a significant cost).

Net: global mode still removes ~60% of main's pass CPU on three.js ×10 with sourcemaps and
~25% without; on wall time it sits between barrier mode and main. The remap was not optimized
(it copies `names` per module and allocates per module).

### Peak RSS

`getrusage(RUSAGE_SELF).ru_maxrss` of a process doing one build, sourcemaps on, median of 5 runs
(min–max):

| bench        | main                | barrier PoC         | global (separate run; main in that run) |
| ------------ | ------------------- | ------------------- | --------------------------------------- |
| three.js ×10 | 1122 MiB (914–1507) | 1026 MiB (906–1614) | 1035 MiB (757–1110); main 1109          |
| rome         | 351 MiB (302–405)   | 326 MiB (308–478)   | 304 MiB (283–347); main 331             |
| antd         | 202 MiB (201–204)   | 152 MiB (151–154)   | 165 MiB (164–167); main 206             |

Global mode adds the concatenated source `V` (≈ the size of all module sources) and its line
tables, which shows on antd (+13 MiB over barrier mode); it still stays below main.

Keeping the module arenas alive through DCE costs less than main's re-parse of the whole chunk
(a second full AST plus the minified text and map), so peak RSS goes down. The parallel cases are
noisy; antd (single-threaded) shows the effect cleanly.

## 4. Output differences vs main

Sizes (sourcemaps on):

| bench        | main         | barrier PoC  | Δ                 | global       | Δ               |
| ------------ | ------------ | ------------ | ----------------- | ------------ | --------------- |
| three.js ×10 | 10,058,896 B | 10,235,869 B | +176,973 (+1.76%) | 10,061,578 B | +2,682 (+0.03%) |
| rome         | 1,830,171 B  | 1,831,889 B  | +1,718 (+0.09%)   | 1,831,733 B  | +1,562 (+0.09%) |
| antd         | 3,733,041 B  | 3,733,038 B  | −3                | 3,733,038 B  | −3              |

`diff -B` between main and global mode: 20 lines on three.js ×10, 15 on rome, 2 on antd, all
category 2 below (headers/licence comments that main drops) — global mode has no code
differences on the three benchmarks.

Categories (`diff -B` and the snapshot suite):

1. **Cross-module single-use inlining blocked by the barrier** — almost all of the size
   difference. On three.js each `*.glsl.js` module (`export default \`…\``) is inlined by main
   into `ShaderChunk`'s object literal. The PoC keeps
   `var alphamap_fragment_glsl_default = …;`and references it. Same for`var require_x = __commonJSMin(…); require_x()`vs main's inlined`(__commonJSMin(…))()`, and
   runtime helpers like `__require`. Semantically equivalent. Global mode removes this category
   entirely.
2. **Module header comments and license comments kept** where main loses them (main's reprint
   drops a comment when the statement it is attached to is rewritten; in the PoC the
   `// path.js` header is joiner text).
3. **Blank lines** between modules (main's reprint removes the joiner's blank lines).
4. **Generated text not re-formatted**: `export * from "x"` without `;`, `'use client'` quote
   style.

Semantic checks:

- `node --check` passes on all PoC outputs (three.js ×10, rome, antd; barrier, `nobarrier` and
  global).
- Sourcemaps (`check_map.cjs`, every mapping): 0 out-of-bounds original positions and 100% of
  named mappings point at that name in the original source, for main, barrier and global mode on
  all three benchmarks. `nobarrier-unsafe` breaks this (rome: 1,026 out-of-bounds mappings, 15
  wrong names), which is why barrier mode needs the barrier.
- Moved code in global mode: per-source mapping counts of global vs barrier mode differ for only
  5 of rome's 1,156 sources, by a few tokens each (the removed `var x =` declarations); the
  inlined object literals keep mapping into their original module. On three.js the inlined GLSL
  literals map to the reference site in `ShaderChunk.js`, exactly as in main (oxc gives the
  inlined expression the span of the identifier it replaces). In `esbuild/default/
  re_export_common_js_as_es6`, code oxc moved from `foo.js` into the chunk's generated tail maps to
  `foo.js`.
- three.js ×10 bundles imported in Node and exercised (export counts, `Vector3`/`Matrix4`/
  `Quaternion` math, geometry, `Scene.toJSON`, every `ShaderChunk` and `ShaderLib` string):
  identical results for main, barrier, `nobarrier` and global mode.

Tests: `cargo test -p rolldown`, `RUST_BACKTRACE` unset.

| run                        | passed | failed                                              | snapshots changed |
| -------------------------- | -----: | --------------------------------------------------- | ----------------: |
| flag off                   |   2181 | 2 (known: `test262_module_code`, `tla/await_using`) |                 0 |
| `ROLLDOWN_CHUNK_AST_DCE=1` |   2181 | same 2                                              |              1105 |
| `=nobarrier`               |   2181 | same 2                                              |              1038 |
| `=global`                  |   2181 | same 2                                              |              1038 |

(After adding the `plugin_sourcemap_chain` fixture: 2182 passed in every mode, same 2 failures;
flag off changes no snapshot; global mode changes the same 1,038.)

The first run with the flag on also failed `issues/1722/{1,2}` (a facade chunk with no modules
whose export text contains `var sub = import_sub.sub;`; the split dropped it) and
`errors/chunk_parse_error` (an invalid `banner` is only caught by the re-parse). Both are fixed
on the branch (tail = everything after the last marker; non-comment banners fall back).

The 1,105 changed snapshots (none accepted), by the worst kind of changed line in each file:

| kind                                          | files |
| --------------------------------------------- | ----: |
| blank lines only                              |   798 |
| + comment lines (headers/license kept)        |    55 |
| + sourcemap visualizer line shifts            |    10 |
| + `export * from` / directive text formatting |    22 |
| code lines changed                            |   220 |

Of the 220 "code" files, 195 lose their code changes under `nobarrier` (category 1). Of the
remaining 25, most are tests with sourcemaps (the barrier stays) or with content hashes in file
names that change because the text changed. The rest are the `nobarrier` cross-module moves
losing `/* @__PURE__ */`, i.e. the reason for the barrier. All `_test.mjs` executions pass in
every mode. In barrier mode the `@__PURE__` counts that drop are helpers no longer inlined out of
the hidden runtime region, not lost annotations.

Global mode's 1,038 changed snapshots (none accepted), by the worst kind of changed line: 910
blank lines only; 89 with comment lines (headers/licence comments kept: 123 comment lines added;
the 15 removed are 14 lines of the test harness's `// HIDDEN [\0rolldown/runtime.js]` marker,
whose placement depends on the blank line the joiner leaves, and one `sourceMappingURL` line that
moved); 22 with `export * from` or directive text formatting; 9 with sourcemap visualizer changes;
7 whose only code-line changes are content hashes in file names (one more file has a change the
classifier does not bucket). No snapshot gains or loses an annotation outside the
sourcemap visualizer text. Two global-mode bugs were found by this suite and fixed: head/tail
printed with comments off lost `/* @__PURE__ */` on code inlined into the generated tail (4
snapshots), and placing generated statements by span moved a merged `export … from` after an
`import` and changed module evaluation order (7 snapshots; `chunk_import_source_order` caught it
only in the snapshot, not in execution).

## Current correctness checkpoint

The measurements above are historical experiment results. The current global path also supports
compression and identifier mangling; its ownership, module metadata and sourcemap contracts are
recorded in `internal-docs/chunk-ast-minify/implementation.md`. Both `oxc_codegen` and
`oxc_minifier` 0.156.0 are vendored with explicit patches. Performance optimization is paused.

The final native build passes the 36-case packed Node API/CLI original/global matrix, including
compress-only, mangle-only, full minify, dynamic imports, exact final-map anchors and fallback
assertions. Node watcher tests pass in both modes (44 each). The Node main suite passes 1,431
cases in original mode; global passes 1,414 with 17 snapshot-test failures (19 reported snapshot
mismatches), retaining the existing expectations. The audit found and fixed an empty producer-map
boundary that incorrectly inherited a preceding foreign source mapping.

The pinned Rollup compatibility harness passes 1,262 cases in original mode. Global passes 1,259
with three exact source-inventory assertion failures after constant folding removes unused lib
sources. Sixteen supplemental runtime/map/ignore-list cases pass. Existing harness ignore lists
and known-failure skips remain active. Source-list identity is not part of the global contract.

Global remains opt-in and still uses the original pipeline for unsupported formats, plugins and
chunk operations. This checkpoint does not establish complete replacement, platform coverage or
sanitizer safety. No Node/Rollup expectations were updated. The earlier reviewed Rust snapshot
updates remain part of the checkpoint. The production checklist below predates this validation;
its full-minify item is now implemented for eligible chunks, while fallback expansion and
maintained dual-path CI remain open.

## 5. What a production version would take, and the risks

Estimate: **3–5 weeks** for an ESM, `dce-only` production version in global mode (size parity,
correct maps), mostly rolldown work plus upstreaming the small `oxc_codegen` API; full minify on
top of that is a separate, larger step (item 4).

1. **Make it the default for covered chunks.** The `renderChunk` gate now uses concrete plugin
   identity; unknown plugins keep falling back. Any future expansion needs a hook contract that
   preserves minify ordering. Module metadata uses post-compression fragment attribution in
   the global path; this accepted compatibility change is documented and regression-tested in
   `metadata_audit` (see internal-docs/chunk-ast-minify/implementation.md).
2. **Cover more of the chunk text as AST**: cjs/iife/umd wrappers, concatenated module groups,
   other wrapped-entry shapes, `intro`/`outro`/banner code (parse them, as the PoC does with the
   import/export text). Or keep falling back.
3. **Size parity (done in the PoC as global mode)**: upstream the `oxc_codegen` API (shared and
   concatenable `LineOffsetTables`); make rebasing cheaper or unnecessary, e.g. by giving each
   module a span offset at parse time so module spans are chunk-unique from the start (then no
   `VisitMut` pass and no copy of the sources into `V`; codegen would only need per-module line
   tables looked up by offset). A codegen-side source resolver (emit `(source, line, col)`
   directly) would also remove the remap pass and the per-module source-content copies.
4. **Full minify** (`minify: true`): global mode is the prerequisite (`sequences`/`join_vars` and
   mangling cross modules all the time). Mangling also needs the module prints to use the chunk
   `Scoping` (names are rewritten in the scoping, not the AST).
5. **Ownership encapsulated**: `owned.rs` consumes eligible ASTs, retains intact donor cells,
   stable scratch storage and shared source text, and exposes only frozen printing. Internal
   lifetime erasure remains audited unsafe code; see internal-docs/chunk-ast-minify/implementation.md.
6. **Tests**: run the whole suite in both modes in CI until the old path is removed; add
   targeted fixtures for the fallbacks and for sourcemap correctness of DCE'd chunks.

Main risks:

- **The finalized AST must be "re-analyzable"**, i.e. equal to what parsing its printed text
  gives. The `ClassType` bug shows rolldown does not guarantee this today: the re-parse hid
  finalizer shortcuts. More may exist (the semantic checks here found only that one), and any
  that remain turn into silent DCE differences or, worse, wrong DCE.
- **oxc minifier invariants**: barrier mode relies on DCE-only mode keeping top-level statement
  order, not removing `export {}`, and not merging statements across it. Global mode relies on
  top-level order and on new statements taking the span of the statement they replace (that is
  how they are assigned to a module). Some passes treat source offsets as execution order (`this`
  before `super()` in one function); rebased spans keep that order. These are not documented
  contracts; an oxc upgrade could break the split. Barrier mode `assert`s on the marker count;
  global mode has no equivalent check (a misassigned statement prints with the wrong module's
  comment table and source name, as in the antd and `export … from` bugs above).
- **Spans as identity**: global mode rewrites every span of every module AST. Anything that reads
  module spans after it (today nothing does; the ASTs are dropped after printing) would see `V`
  offsets.
- **Plugins and observability**: `renderChunk`/`RenderedChunk.modules[].code` semantics change.
- **Memory** is fine for the measured cases, but it depends on the module arenas being smaller
  than a re-parse of the chunk, which holds for typical code.
- **Parallelism**: DCE stays serial per chunk. For the big single-chunk cases it is now most of
  the remaining minify cost (≈290 ms on three.js ×10); further gains need a parallel or
  incremental DCE in oxc.

## Files

- `crates/rolldown/src/stages/generate_stage/chunk_ast_dce/mod.rs` — the fast path (barrier
  mode, eligibility, generated head/tail).
- `crates/rolldown/src/stages/generate_stage/chunk_ast_dce/global.rs` — global mode: source
  space, span rebase, split by span, printing against `V`, sourcemap remap and source dedupe.
- `vendor/oxc_codegen`, `vendor/oxc_codegen.patch` — the patched `oxc_codegen` 0.156.0.
- `crates/rolldown/src/ecmascript/ecma_generator.rs` — dedupes the joined map's sources in global
  mode.
- `crates/rolldown/src/utils/render_ecma_module.rs`, `crates/rolldown_common/src/types/
  module_render_output.rs` — `map_is_final` for maps composed by global mode.
- `crates/rolldown/tests/rolldown/topics/chunk_ast_dce/plugin_sourcemap_chain` — fixture for moved
  code out of plugin-transformed modules (snapshot taken with the flag off; compare it with
  `ROLLDOWN_CHUNK_AST_DCE=global`).
- `crates/rolldown/src/stages/generate_stage/render_chunk_to_assets.rs` — runs it before module
  printing, consumes eligible ASTs into frozen chunk owners.
- `crates/rolldown/src/ecmascript/format/esm.rs` — uses the re-printed imports/exports.
- `crates/rolldown/src/stages/generate_stage/minify_chunks.rs` — skips fast-path chunks; timed copy
  of `dce_or_minify` under `ROLLDOWN_POC_PROF`.
- `crates/rolldown_common/src/module/normal_module.rs` — `render_with(…, force_annotation)`.
- `crates/rolldown/src/module_finalizers/mod.rs` — the `ClassType` fix.
- `crates/rolldown_utils/src/poc_prof.rs` — phase CPU accounting.
- `crates/bench/examples/minify_prof.rs` — benchmark driver (adds max RSS, phase report,
  `rome:<entry>` input); `crates/bench/examples/dce_text.rs` — runs oxc DCE on a text file.

oxc changes: barrier mode none; global mode the `oxc_codegen` patch above (no `oxc_minifier`
changes).
