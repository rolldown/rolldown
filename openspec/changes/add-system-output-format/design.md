## Context

Rolldown's output format pipeline is a flat, match-based dispatch:
`OutputFormat` is a 4-variant enum; `EcmaGenerator::instantiate_chunk` matches
on it and calls one of four free functions (`render_esm`, `render_cjs`,
`render_iife`, `render_umd`). Each free function builds a `SourceJoiner` by
emitting format-specific preamble, the module sources, and a format-specific
postamble. Roughly 20 additional `match` branches on `OutputFormat` are
scattered across the link and generate stages.

The module finalizer (`crates/rolldown/src/module_finalizers/mod.rs`, ~2500
lines) walks each module's AST and rewrites nodes for the target format.
Currently, live binding support for CJS is achieved by wrapping export
_declarations_ in `Object.defineProperty` getters — no per-assignment-site
instrumentation is needed because the getter resolves lazily at read time.

SystemJS is structurally different: the runtime must be notified of every
binding mutation synchronously at the mutation site. This requires a new class
of AST transformation — per-assignment `exports()` wrapping — that has no
precedent in the existing finalizer.

Rollup's reference implementation (`rollup/src/finalisers/system.ts`) is 254
lines because the binding-tracking infrastructure is shared. Rolldown's
equivalent will require more because the per-assignment transformation must be
built into the finalizer from scratch.

The binding layer already has `systemNullSetters` commented out
(`crates/rolldown_binding/src/options/binding_output_options/mod.rs:139`),
confirming that this format was intentionally deferred, not forgotten.

## Goals / Non-Goals

**Goals:**

- Full `System.register` output compatible with SystemJS 6.x (the current
  production release), including named and anonymous registration, ordered
  setters, and the `execute` / async-execute split.
- Per-assignment live export instrumentation covering all assignment forms
  present in valid JavaScript: simple assignment, compound assignment (`+=`,
  `-=`, …), prefix and postfix increment/decrement, destructuring assignment,
  `for`/`for..of`/`for..in` loop variables that are exported, and function/class
  declaration hoisting.
- Native code splitting: `module.import()` for dynamic imports; no forced
  `codeSplitting: false` (unlike IIFE/UMD).
- `module.meta` / `module.meta.url` rewriting for import meta.
- `_starExcludes` for `export *` semantics.
- `systemNullSetters` option (default `true`, matching Rollup v4 default).
- `output.name` for named `System.register`.
- Rollup test compatibility: all 16 currently-skipped SystemJS fixtures must
  pass.

**Non-Goals:**

- AMD format (separate effort, separate format variant).
- SystemJS 0.x / 2.x compatibility (pre-`System.register` API).
- Source phase imports (`import source x from 'y'`) — hard error, same as
  Rollup.
- `output.compact` mode (a separate unimplemented feature in rolldown).
- `output.interop` option (a separate unimplemented feature in rolldown).

## Decisions

### Decision 1: Match-arm extension, not a new abstraction

**Choice:** Add `OutputFormat::System` as a fifth variant and extend all
existing `match` branches. Do not introduce a format trait or strategy object.

**Rationale:** The existing pattern — free functions per format, shared
`GenerateContext` — is consistent throughout the codebase and deliberately flat.
A new abstraction would require refactoring all four existing formats and would
not be justified by a single new variant. Adding a fifth arm to ~20 match sites
is mechanical and low-risk.

**Alternative considered:** A `FormatRenderer` trait implemented per format.
Rejected: adds abstraction overhead and requires touching every existing format
for no benefit.

### Decision 2: New `ExportsKind` / live-export tracking flag in the finalizer

**Choice:** Introduce a per-module boolean flag `has_system_live_exports: bool`
(or reuse/extend an existing flag) computed during the link stage, then consumed
in the finalizer to decide whether to inject `exports()` wrappers.

**Rationale:** The finalizer already gate-keeps format-specific behaviour behind
`ctx.options.format` checks. For SystemJS, every exported mutable binding
assignment needs wrapping; the set of such bindings is already known at link
time via `SymbolRefDb`. Flagging which modules have SystemJS-relevant exported
symbols avoids scanning every assignment in every module unconditionally.

**Alternative considered:** Compute on-the-fly during finalizer traversal by
checking `options.format == System && symbol is exported`. Viable but requires a
secondary `symbol_db` lookup on every assignment-related AST node visit, adding
overhead for all non-system builds. The pre-computed flag is zero-cost for other
formats.

### Decision 3: `exports()` injection shape

**Choice:** Follow Rollup's proven injection patterns exactly:

- Initializer: `let x = exports("x", <init>);`
- Reassignment: `exports("x", x = <rhs>);`
- Compound: `exports("x", x += <rhs>);`
- Postfix `x++`: `exports("x", x + 1), x++;` (export old+1, then actually
  increment) — wait, actually Rollup emits `exports("x", (x++, x))` ... confirm
  with fixture.
- Prefix `++x`: `exports("x", ++x);`
- Batch (multiple at once): `exports({ a: a, b: b });`
- Hoisted function: prepend `exports("fn", fn)` before execute body.

**Rationale:** Rollup's patterns are correct by construction and battle-tested.
The 16 existing rollup fixtures assert exact output; matching them gives a free
correctness bar. Diverging would require maintaining our own correctness proofs.

**Note:** The exact `x++` postfix form must be verified against the Rollup
fixture `system-export-rendering` before implementation.

### Decision 4: Setter generation from chunk dependency graph

**Choice:** Build the `setters[]` array during the generate stage (in
`format/system.rs`) by iterating the chunk's cross-chunk import records in the
same order they appear in the deps array. Null setters (`null` vs
`function(){}`) controlled by `systemNullSetters` option (default `true`).

**Rationale:** The chunk dependency graph is already fully resolved at generate
time. The deps array and setters array must be in the same order — generating
both from the same ordered iteration of `chunk.imports_from_other_chunks` /
external imports guarantees alignment by construction.

### Decision 5: `module` parameter deconfliction

**Choice:** Add `"module"` and `"exports"` to the reserved names set for
SystemJS format in `renamer.rs` (analogous to how CJS adds `"module"`,
`"require"`, `"exports"`). Any user variable named `module` or `exports` will be
renamed to `module$1` / `exports$1` etc. by the existing deconfliction pass.

**Rationale:** The factory function's `(exports, module)` parameters occupy the
top scope. If user code has a local `module` variable, it must be renamed before
any other transformation. The existing `deconflict_chunk_symbols.rs` pass
handles this if we seed the reserved-names set correctly.

### Decision 6: Code splitting is enabled

**Choice:** Do NOT add `OutputFormat::System` to the
forced-`codeSplitting: false` guard in `prepare_build_context.rs` /
`validate_options_for_multi_chunk_output.rs`.

**Rationale:** SystemJS is the only non-ESM format with native code-splitting
support (`module.import()` is a first-class async load, not a shim). Disabling
it would remove the primary reason to use SystemJS over IIFE/UMD. The ESM
code-splitting infrastructure already exists; the only change needed is using
`module.import()` instead of `import()` in the generated code.

### Decision 7: No new runtime helpers in `runtime-base.js`

**Choice:** Do not add SystemJS-specific runtime helpers to rolldown's runtime
bundle. Any inline helpers (e.g., `_mergeNamespaces`) are emitted directly into
the chunk by `format/system.rs` when needed, just as Rollup does.

**Rationale:** SystemJS output does not need shared cross-chunk runtime helpers
— the SystemJS runtime itself provides the execution environment. The only
inline helper that may be needed (`_mergeNamespaces`, for
`export * from external`) is rare and short enough to inline directly. Adding it
to `runtime-base.js` would needlessly bloat non-SystemJS builds.

## Risks / Trade-offs

**[Risk] Per-assignment `exports()` injection is novel in the finalizer** → The
finalizer has never done per-assignment-site code injection. Compound
assignments (`+=`, `&&=`, `||=`, `??=`), destructuring patterns
(`let [a, b] = ...`), and for-loop variable exports each require distinct
handling. Mitigation: implement against Rollup's 16 fixture outputs as the
acceptance test; add rolldown-specific fixtures for cases not covered by the
Rollup suite.

**[Risk] Postfix increment/decrement export semantics are subtle** → `x++`
should export the post-increment value but the exact pattern differs from
prefix. Mitigation: verify exact output against Rollup's
`system-export-rendering` fixture before writing the transformation.

**[Risk] Setter order must exactly match deps array order** → Any mismatch
between the deps array and the setters array is a silent runtime bug (wrong
setter fires for wrong dep). Mitigation: generate both arrays in a single pass
over the same ordered data structure; add a debug assertion in dev builds that
`deps.len() == setters.len()`.

**[Risk] `export *` with overlapping names from multiple sources** → The
`_starExcludes` null-proto object must include all own export names including
those from other star-imports that have been resolved. Missing an entry causes a
local export to be accidentally shadowed. Mitigation: collect all own export
names (including resolved re-exports) before building `_starExcludes`.

**[Risk] Interaction with `keep_names` and const-inlining** → The finalizer
performs const inlining and `keep_names` wrapping before some assignment
rewrites. Exported consts that are inlined do not need `exports()` wrapping; the
`must_keep_live_binding` logic already handles this for CJS and should be
reused/extended for SystemJS. Mitigation: reuse the existing
`must_keep_live_binding` predicate; add test cases for inlined-const exports.

**[Trade-off] No `output.compact` support initially** → Rollup's
`system-export-rendering-compact` fixture tests compact output. Since
`output.compact` is not implemented in rolldown at all, this fixture will remain
skipped. This is acceptable — compact is a separate feature gap.

## Open Questions

1. **Postfix `x++` export form**: Rollup emits `exports("x", (x++, x))` — is
   this actually correct? It exports the post-increment value, which requires
   reading `x` after the `++`. Needs verification against the
   `system-export-rendering` fixture before finalizing the transformation shape.

2. **Destructuring export**: `let [a, b] = fn()` where both `a` and `b` are
   exported — does Rollup emit a batch `exports({ a, b })` call after the
   destructuring, or does it transform the destructuring itself? Needs fixture
   inspection.

3. **TLA (top-level await) and async execute**: SystemJS supports
   `execute: async
function() { ... }`. Rolldown has TLA support for ESM. The
   path to reuse/extend that for SystemJS needs to be traced during
   implementation.

4. **`module` parameter presence heuristic**: The `module` parameter is only
   emitted when the chunk uses `module.import()` or `module.meta`. How does
   rolldown currently track which chunks use dynamic import / import.meta? This
   tracking needs to gate the `module` parameter emission.
