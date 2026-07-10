# Pass-Based Pipeline — Design & Principles

## Summary

A guiding methodology for structuring bundler-internal pipelines (stage-level dataflow) as passes with a compile-time ownership contract: every pass implements one small trait declaring what it reads, owns, seals, and hands onward. Three compiler gates pin the shape; what they cannot pin is a short, explicit review list (see [Enforcement](#enforcement)). This doc defines the contract; there is no implementation.md yet — the first flow that adopts it adds one.

## Ground rules (read this first)

- This is a methodology, **not a migration mandate**. Do **not** proactively refactor existing pipeline code into passes.
- Use the pass mechanism only when a maintainer explicitly asks for a pass-based refactor of a flow, or when designing a **new** flow.
- It applies to pipeline top-level structure only. Helpers, visitors, and utilities stay plain functions — do not try to make everything a pass.
- Granularity test: if you cannot name the artifact a step produces (or the working data it transforms), it is not a pass.

## What it is

- A pass is one pipeline step with a machine-checked contract, written as an implementation of one small trait — every pass reads the same way.
- The contract declares four things:
  - what it only reads
  - what it takes ownership of
  - what it seals forever
  - what it hands onward, still mutable
- Enforcement is layered, mostly on ordinary Rust:
  - order: the driver's `let`-chain — a step cannot name inputs that do not exist yet
  - sealing: distinct draft/frozen artifact types — the frozen one is built from frozen representations (`Box<[T]>`, `IndexBox`), so the mutators do not exist
  - ownership: by-value slots — "to modify is to own and hand back"
  - shape: trait bounds — reads cannot be `&mut`, and a pass value cannot carry pipeline state

## What it looks like

The whole mechanism is one trait and one wrapper function:

```rust
/// A pass type is a **name**, not a value: `run_pass` compile-time-asserts it is
/// zero-sized, and the bounds keep it lifetime-free. All runtime data — including
/// configuration — enters through the declared slots.
pub trait Pass: Copy + 'static {
  type InputRead<'a>: Copy;         // shared borrows only; Copy makes `&mut` unrepresentable here
  type InputOwned;                  // data taken over (to modify = to own and hand back); `()` if none
  type OutputRead;                  // minted here, frozen by representation (see below)
  type OutputOwned;                 // still-mutable data handed to a later pass

  async fn run(self, cx: &mut PassCtx, read: Self::InputRead<'_>, owned: Self::InputOwned)
    -> BuildResult<(Self::OutputRead, Self::OutputOwned)>;
}

pub async fn run_pass<P: Pass>(pass: P, cx: &mut PassCtx, read: P::InputRead<'_>, owned: P::InputOwned)
  -> BuildResult<(P::OutputRead, P::OutputOwned)> {
  const { assert!(size_of::<P>() == 0, "a pass is a name — state lives in run() locals or in the slots") };
  // tracing span + diagnostics provenance live here, once, for every pass
  pass.run(cx, read, owned).await
}
```

Conventions:

- The passes module carries `#![forbid(unsafe_code)]`, which closes the raw-pointer residual of the two `Pass` bounds.
- `self` carries **nothing**: every pass is a zero-sized name token (`const`-asserted in `run_pass`), kept only so call sites read as `run_pass(OptimizeChunksPass, ..)`. Configuration is runtime data and enters through `InputRead` like everything else.
- Slot types: empty = `()`; a single artifact = the bare type; multiple = a named per-pass struct (`XxxPassInputRead { .. }`, derives `Copy`), which doubles as the pass's greppable dependency manifest.
- Naming: a pass type ends in `Pass` (`OptimizeChunksPass`); its input struct is `<PassName>InputRead`. Since passes are zero-sized name tokens, the suffix keeps call sites unambiguous — and `rg 'struct \w+Pass;'` is the complete pass inventory.
- `PassCtx` is the single sanctioned `&mut`: write-only sinks (diagnostics now, devtools trace later). It never contains pipeline data; passes may write it but never read it. `run_pass` (the wrapper) owns tracing spans and stamps every diagnostic with the emitting pass's name.
- Driver rules: every `OutputOwned` is consumed by exactly one later `InputOwned` (or explicitly dropped). **Seal order follows reference direction**: seal an artifact only when everything its keys/indices point into is already sealed.

The freeze boundary proven in-tree today is `UsedSymbolRefsBuilder::seal()` in the generate stage: source liveness becomes read-only while chunk layout stays mutable — freeze lines are per-artifact, not global:

```text
UsedSymbolRefsBuilder ──(mutated through chunk generation)──► seal() ──► UsedSymbolRefs (frozen)
                                                (chunk graph stays mutable past this point)
```

What a fully adopted flow could look like — **hypothetical**: today's `GenerateStage` does not have this boundary (its graph keeps being mutated by link derivation, wrapping, naming, and finalization well past chunk optimization). Sealing the chunk graph itself is left to the first flow that adopts this methodology:

```text
()  ──Split──►  Draft  ──Optimize──►  Draft  ──Seal──►  ChunkGraph (frozen)
                                                             │
                                 ┌──────── &ChunkGraph ──────┤
                                 ▼                           ▼
                            ComputeLinksPass                AssignNamesPass
                                 │                           │
                            Links (sealed)             Names (sealed)
```

## Example

Three passes, three roles — the slots mirror what each pass does to the data (names are illustrative; `DraftChunkGraph`, `ChunkNames` etc. do not exist today):

```rust
// 1) Reshape: chunk merging restructures the graph itself, so it owns the graph.
#[derive(Clone, Copy)]
pub struct OptimizeChunksPass;

#[derive(Clone, Copy)]
pub struct OptimizeChunksPassInputRead<'a> {
  pub modules: &'a ModuleTable,
  pub metas: &'a IndexVec<ModuleIdx, LinkingMetadata>,
}

impl Pass for OptimizeChunksPass {
  type InputRead<'a> = OptimizeChunksPassInputRead<'a>;
  type InputOwned    = DraftChunkGraph;   // it merges chunks and moves modules
  type OutputRead    = ();
  type OutputOwned   = DraftChunkGraph;   // handed back, still mutable

  async fn run(self, _cx: &mut PassCtx, read: Self::InputRead<'_>, mut graph: Self::InputOwned)
    -> BuildResult<((), Self::OutputOwned)> {
    // we own `graph`: mutate freely, internal `par_iter_mut` is fine
    Ok(((), graph))
  }
}

// 2) Seal: the freeze transition is itself a pass — compact, then freeze the
//    representation: IndexVec<ChunkIdx, Chunk> becomes IndexBox<ChunkIdx, [Chunk]>.
#[derive(Clone, Copy)]
pub struct SealChunkGraphPass;

impl Pass for SealChunkGraphPass {
  type InputRead<'a> = ();
  type InputOwned    = DraftChunkGraph;
  type OutputRead    = ChunkGraph;        // fields are IndexBox / Box<[_]> — mutation is unrepresentable
  type OutputOwned   = ();
  // ...
}

// 3) Derive: the most common shape — owns nothing, reads sealed data, mints a new sealed artifact.
#[derive(Clone, Copy)]
pub struct AssignNamesPass;

impl Pass for AssignNamesPass {
  type InputRead<'a> = AssignNamesPassInputRead<'a>;   // { graph: &'a ChunkGraph, options: &'a NormalizedBundlerOptions }
  type InputOwned    = ();
  type OutputRead    = ChunkNames;        // e.g. IndexBox<ChunkIdx, [ArcStr]>, frozen at birth
  type OutputOwned   = ();
  // ...
}
```

The driver is a typed `let`-chain (deliberately **not** `Vec<Box<dyn Pass>>` — heterogeneous signatures are the point):

```rust
let ((), graph) = run_pass(OptimizeChunksPass, &mut cx, optimize_reads, graph).await?;
let (graph, ()) = run_pass(SealChunkGraphPass, &mut cx, (), graph).await?;
let (names, ()) = run_pass(AssignNamesPass, &mut cx, names_reads, ()).await?;
```

Needing to own more than you reshape is the signal that an artifact should be split out — not a reason to widen the slot.

## Why

Wrong order is a compile error, not a comment:

```rust
let (canon, ()) = run_pass(DeconflictPass, &mut cx, DeconflictPassInputRead { names: &names, /* .. */ }, ()).await?;
let (names, ()) = run_pass(AssignNamesPass, &mut cx, names_reads, ()).await?;
// error[E0425]: cannot find value `names` in this scope
```

Sealed means sealed:

```rust
graph.add_chunk(chunk);
// error[E0599]: no method named `add_chunk` found for struct `ChunkGraph`
// (mutators exist only on DraftChunkGraph)
```

### Parallelism: signatures expose the candidates, the compiler checks the join

Two passes are concurrency candidates exactly when their `InputOwned`s are disjoint and neither reads the other's output — both facts sit in the slot types. What each layer actually guarantees:

- **Signatures** expose the candidates: disjoint owned data, no artifact dependency between the two.
- **The borrow checker (plus `Send`/`Sync`)** proves the join is free of data races — an unsound join (shared owned data, missing artifact) fails to compile.
- **Semantic independence is not proven; it is a stated discipline**: no interior mutability in pipeline data, no globals, no order-dependent external calls (plugin hooks, I/O) inside candidate passes, and effects only through per-branch `PassCtx` sinks that the driver merges in a fixed order. Under those rules — and only under them — a compiling join is also deterministic.

```rust
// ComputeLinksPass : InputRead = (&ChunkGraph, &SymbolRefDb), InputOwned = ()
// AssignNamesPass  : InputRead = (&ChunkGraph, &Options),     InputOwned = ()
let (links, names) = try_join!(
  run_pass(ComputeLinksPass, &mut cx_a, (&graph, &symbols), ()),
  run_pass(AssignNamesPass,  &mut cx_b, (&graph, &options), ()),
)?;
```

### Other benefits

The dependency graph is greppable — impact analysis without reading bodies:

```console
$ rg 'symbol_db: &' -g '*.rs'                 # every pass that reads the symbol table
$ rg 'type InputOwned    = DraftChunkGraph'   # every pass that ever owns the graph
```

Each pass is unit-testable by construction: its `InputRead` type is the exact, minimal fixture spec — no need to build whole stage outputs to test one pass.

Uniform machinery: `run_pass` is the single home for tracing spans and diagnostics provenance (`type_name::<P>()`), so observability never needs per-pass wiring.

## Enforcement

The goal is **not** to make illegal states unrepresentable — it is to make them impossible to write _quietly_. State has exactly three legal homes: locals inside `run` (unrestricted); driver-built values lent through `InputRead`; artifacts moving through the owned slots. There is no fourth place — a pass type itself is zero-sized, so "pass-internal state" is not a category that exists. What therefore has no home is state that crosses passes without appearing in any signature. The gates below force exactly that case into the open, where review can catch it — under a shared `&mut` world it was invisible by construction.

What the compiler pins:

- reads cannot be `&mut` — `InputRead<'a>: Copy` (`&mut` is never `Copy`)
- a pass value cannot carry anything at all — `run_pass` `const`-asserts `size_of::<P>() == 0`; `Pass: Copy + 'static` additionally rules out lifetime-carrying zero-sized tricks
- no escape through raw pointers — `#![forbid(unsafe_code)]` on the passes module
- pass order — `let`-chain scoping; sealing — frozen representations (the mutators do not exist); ownership transfer — moves

What stays review-held (the honest list):

- a sealed struct's fields are actually frozen representations — a one-look check on the type definition; the representation is the proof, there is no marker trait to keep honest
- no interior mutability in pipeline data types; no global statics holding pipeline data (the one remaining way to smuggle state past the bounds)
- `PassCtx` used write-only; artifact granularity (own exactly what you reshape)

Why a trait at all: uniform pass style — every pass declares the same four slots and is invoked through one wrapper — plus the shape gates above, which plain functions cannot carry (nothing stops an extra `&mut` parameter on a plain function except review). The cost — GATs and `()`/tuple ceremony — is accepted deliberately.

## Future directions

- Driver-level `try_join!` of parallel-candidate passes, once profiling shows a win worth taking.
- Incremental cache friendliness: explicit inputs are natural dependency keys, sealed artifacts are natural snapshot/hash units, and a pass is a natural recompute unit. To be honest: the contract was **not** designed with incremental builds as a premise, and nothing in it depends on that — it simply does not stand in the way.

## Related

- `implementation.md` — none yet; added by the first flow that adopts the contract.
