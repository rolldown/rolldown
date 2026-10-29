# PoC: chunk-level DCE on the module ASTs instead of a chunk re-parse

Branch `poc/chunk-ast-minify`. Not meant to be merged. The question: how much time do we save if
the chunk-level `dce-only` pass runs on an AST assembled from the finalized module ASTs, instead
of printing every module, joining the text and re-parsing the whole chunk in `minify_chunks`?

**Short answer:** with sourcemaps on, the pass drops from 675 → 289 ms CPU on three.js ×10
(−57%), 83 → 41 ms on rome, 170 → 50 ms on antd. Because the pass is serial on a single big
chunk, wall time follows: three.js ×10 builds go from ~1.45 s to ~0.76 s (minify off: ~0.41 s).
Peak RSS does not rise; it drops slightly. Output is semantically equivalent on everything
checked, but up to 1.8% larger, because a module barrier blocks oxc's single-use inlining across
module boundaries. Removing that size cost needs per-node source context in oxc codegen.

Everything is behind an env var, so main's behavior and the PoC are compared from one build:

| Env var                                   | Effect                                                              |
| ----------------------------------------- | ------------------------------------------------------------------- |
| `ROLLDOWN_CHUNK_AST_DCE=1`                | fast path, with module barrier (correct sourcemaps)                 |
| `ROLLDOWN_CHUNK_AST_DCE=nobarrier`        | no barrier when sourcemaps are off (measurement only, see §4)       |
| `ROLLDOWN_CHUNK_AST_DCE=nobarrier-unsafe` | no barrier even with sourcemaps (measurement only, maps get broken) |
| `ROLLDOWN_POC_PROF=1`                     | per-phase thread-CPU accounting, printed by `minify_prof`           |
| `ROLLDOWN_CHUNK_AST_DCE_DEBUG=1`          | eligibility / iteration-count logging                               |
| `ROLLDOWN_CHUNK_AST_DCE_DUMP=<dir>`       | write the assembled chunk program (printed) before DCE              |

With the env var unset, output is byte-identical to main (checked on three.js ×10, rome, antd,
and the full `cargo test -p rolldown` snapshot suite).

## 1. Design: codegen and sourcemaps

**Chosen: "DCE on the assembled AST, then print each module with its own context".** No oxc
changes were needed; nothing is patched or vendored.

In `create_chunk_to_codegen_ret_map` (before modules are printed), for each eligible chunk
(`crates/rolldown/src/stages/generate_stage/chunk_ast_dce.rs`):

1. Every module's top-level statements are moved (`take_in`, no copy) into one chunk `Program`
   whose body `Vec` lives in a fresh per-chunk allocator. Nodes stay in their module arenas;
   lifetimes are erased with `transmute` (all arenas outlive the chunk program; the chunk
   allocators are dropped after `ast_table`).
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

**Rejected alternative: one codegen pass over the whole chunk with per-segment source context.**
That would allow cross-module inlining with correct maps. It needs an oxc codegen change: either
codegen that switches source text/comments/line tables per span, or rebasing all spans into one
virtual space (an extra full-AST visitor) plus a new token→module remap with the module
`sourcemap_chain`s. It also makes the final print serial (one codegen of the whole chunk instead
of N parallel module prints), so it would be slower than the chosen design on single-chunk builds.

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
- `renderChunk` plugins, except `builtin:asset-module`, `builtin:copy-module` (placeholder string
  rewriting) and the test harness's `hidden-module-markers` (comment lines only) — allowlisted
  **by name**, a PoC hack;
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

### Peak RSS

`getrusage(RUSAGE_SELF).ru_maxrss` of a process doing one build, sourcemaps on, median of 5 runs
(min–max):

| bench        | main                | PoC                 |
| ------------ | ------------------- | ------------------- |
| three.js ×10 | 1122 MiB (914–1507) | 1026 MiB (906–1614) |
| rome         | 351 MiB (302–405)   | 326 MiB (308–478)   |
| antd         | 202 MiB (201–204)   | 152 MiB (151–154)   |

Keeping the module arenas alive through DCE costs less than main's re-parse of the whole chunk
(a second full AST plus the minified text and map), so peak RSS goes down. The parallel cases are
noisy; antd (single-threaded) shows the effect cleanly.

## 4. Output differences vs main

Sizes (sourcemaps on):

| bench        | main         | PoC          | Δ                 |
| ------------ | ------------ | ------------ | ----------------- |
| three.js ×10 | 10,058,896 B | 10,235,869 B | +176,973 (+1.76%) |
| rome         | 1,830,171 B  | 1,831,889 B  | +1,718 (+0.09%)   |
| antd         | 3,733,041 B  | 3,733,038 B  | −3                |

Categories (`diff -B` and the snapshot suite):

1. **Cross-module single-use inlining blocked by the barrier** — almost all of the size
   difference. On three.js each `*.glsl.js` module (`export default \`…\``) is inlined by main
   into `ShaderChunk`'s object literal. The PoC keeps
   `var alphamap_fragment_glsl_default = \`…\`;`and references it. Same for`var require_x = __commonJSMin(…); require_x()`vs main's inlined`(__commonJSMin(…))()`, and
   runtime helpers like `__require`. Semantically equivalent. With `nobarrier` and maps off, the
   three.js ×10 output is within +2,682 B (0.03%) of main and rome within +1,445 B, and the
   remaining differences are only categories 2–4.
2. **Module header comments and license comments kept** where main loses them (main's reprint
   drops a comment when the statement it is attached to is rewritten; in the PoC the
   `// path.js` header is joiner text).
3. **Blank lines** between modules (main's reprint removes the joiner's blank lines).
4. **Generated text not re-formatted**: `export * from "x"` without `;`, `'use client'` quote
   style.

Semantic checks:

- `node --check` passes on all PoC outputs (three.js ×10, rome, antd; also `nobarrier`).
- Sourcemaps (`check_map.cjs`, every mapping): 0 out-of-bounds original positions and 100% of
  named mappings point at that name in the original source, for main and PoC on all three
  benchmarks. `nobarrier-unsafe` breaks this (rome: 1,026 out-of-bounds mappings, 15 wrong
  names), which is why the barrier stays.
- three.js ×10 bundles imported in Node and exercised (export counts, `Vector3`/`Matrix4`/
  `Quaternion` math, geometry, `Scene.toJSON`, every `ShaderChunk` and `ShaderLib` string):
  identical results for main, PoC and `nobarrier`.

Tests: `cargo test -p rolldown`, `RUST_BACKTRACE` unset.

| run                        | passed | failed                                              | snapshots changed |
| -------------------------- | -----: | --------------------------------------------------- | ----------------: |
| flag off                   |   2181 | 2 (known: `test262_module_code`, `tla/await_using`) |                 0 |
| `ROLLDOWN_CHUNK_AST_DCE=1` |   2181 | same 2                                              |              1105 |
| `=nobarrier`               |   2181 | same 2                                              |              1038 |

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

## 5. What a production version would take, and the risks

Estimate: **2–4 weeks** for an ESM, `dce-only` production version with the barrier, plus roughly
the same again to get size parity, which needs oxc work.

1. **Make it the default for covered chunks.** Replace the name-based `renderChunk` allowlist with
   a hook contract (e.g. a plugin flag "my renderChunk only rewrites strings/comments", or run
   `renderChunk` after the DCE as it happens today on the fast path). Decide what `RenderedChunk`
   module code and `renderChunk` should see (post-DCE code is a visible change).
2. **Cover more of the chunk text as AST**: cjs/iife/umd wrappers, concatenated module groups,
   other wrapped-entry shapes, `intro`/`outro`/banner code (parse them, as the PoC does with the
   import/export text). Or keep falling back.
3. **Recover the size loss (cross-module inlining)** in one of two ways. (a) Let oxc codegen
   resolve a node's source context per span: rebase spans per module into one space and give
   codegen a "span → (source, comments, line table)" resolver. Then the chunk can be printed
   either as one program or per module, with moved nodes mapping correctly. (b) Have oxc's
   minifier report or forbid moves across "segment" boundaries, and only allow the cheap,
   span-independent ones (e.g. literals without comments) to cross. (a) is the general fix and
   also covers full minify (mangling, `sequences`, `join_vars` cross modules all the time).
4. **Full minify** (`minify: true`): needs (3a). Mangling also needs the module prints to use the
   chunk `Scoping` (names are rewritten in the scoping, not the AST).
5. **Clean ownership instead of `transmute`**: a safe way to borrow statements from many arenas
   into one program (e.g. an oxc API for "program over foreign arenas", or allocating all module
   ASTs of a chunk in one arena from the start).
6. **Tests**: run the whole suite in both modes in CI until the old path is removed; add
   targeted fixtures for the fallbacks and for sourcemap correctness of DCE'd chunks.

Main risks:

- **The finalized AST must be "re-analyzable"**, i.e. equal to what parsing its printed text
  gives. The `ClassType` bug shows rolldown does not guarantee this today: the re-parse hid
  finalizer shortcuts. More may exist (the semantic checks here found only that one), and any
  that remain turn into silent DCE differences or, worse, wrong DCE.
- **oxc minifier invariants**: the PoC relies on DCE-only mode keeping top-level statement order,
  not removing `export {}`, and not merging statements across it, and on some passes treating
  source offsets as execution order (`this` before `super()` in one function, which stays inside
  one module). These are not documented contracts; an oxc upgrade could break the split. The
  `assert` on the marker count guards the worst case.
- **Plugins and observability**: `renderChunk`/`RenderedChunk.modules[].code` semantics change.
- **Memory** is fine for the measured cases, but it depends on the module arenas being smaller
  than a re-parse of the chunk, which holds for typical code.
- **Parallelism**: DCE stays serial per chunk. For the big single-chunk cases it is now most of
  the remaining minify cost (≈290 ms on three.js ×10); further gains need a parallel or
  incremental DCE in oxc.

## Files

- `crates/rolldown/src/stages/generate_stage/chunk_ast_dce.rs` — the fast path.
- `crates/rolldown/src/stages/generate_stage/render_chunk_to_assets.rs` — runs it before module
  printing, keeps the chunk allocators alive until after `ast_table`.
- `crates/rolldown/src/ecmascript/format/esm.rs` — uses the re-printed imports/exports.
- `crates/rolldown/src/stages/generate_stage/minify_chunks.rs` — skips fast-path chunks; timed copy
  of `dce_or_minify` under `ROLLDOWN_POC_PROF`.
- `crates/rolldown_common/src/module/normal_module.rs` — `render_with(…, force_annotation)`.
- `crates/rolldown/src/module_finalizers/mod.rs` — the `ClassType` fix.
- `crates/rolldown_utils/src/poc_prof.rs` — phase CPU accounting.
- `crates/bench/examples/minify_prof.rs` — benchmark driver (adds max RSS, phase report,
  `rome:<entry>` input); `crates/bench/examples/dce_text.rs` — runs oxc DCE on a text file.

oxc changes: none.
