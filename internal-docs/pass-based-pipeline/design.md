# Pass-Based Pipeline — Design & Principles

## Summary

A guiding methodology for structuring bundler-internal pipelines (stage-level dataflow) as passes with a compile-time ownership contract: every pass implements one small trait declaring what it reads, owns, seals, and hands onward. Compiler gates pin the shape; what they cannot pin is a short, explicit review list (see [Enforcement](#enforcement)). This doc defines the contract; there is no implementation.md yet — the first flow that adopts it adds one.

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
  - sealing: `run_pass` wraps every read-side output in `Sealed<T>` — only `&T` ever comes out; frozen representations inside (`Box<[T]>`, `IndexBox`) are the soft second layer
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
  type OutputRead;                  // minted here; `run_pass` wraps it in `Sealed<_>` — frozen unconditionally
  type OutputOwned;                 // still-mutable data handed to a later pass

  async fn run(self, cx: &mut PassCtx, read: Self::InputRead<'_>, owned: Self::InputOwned)
    -> BuildResult<(Self::OutputRead, Self::OutputOwned)>;
}

pub async fn run_pass<P: Pass>(pass: P, cx: &mut PassCtx, read: P::InputRead<'_>, owned: P::InputOwned)
  -> BuildResult<(Sealed<P::OutputRead>, P::OutputOwned)> {
  const { assert!(size_of::<P>() == 0, "a pass is a name — state lives in run() locals or in the slots") };
  // tracing span + diagnostics provenance live here, once, for every pass
  let (minted, owned_out) = pass.run(cx, read, owned).await?;
  Ok((Sealed::new(minted), owned_out)) // the harness seals — there is no unsealed exit
}

/// The hard freeze: only `&T` ever comes out — no `DerefMut`, no `into_inner`,
/// private field. No mutation path exists, even for an owner.
pub struct Sealed<T>(T);

impl<T> Sealed<T> {
  pub fn new(value: T) -> Self {
    Self(value)
  }
}

impl<T> std::ops::Deref for Sealed<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.0
  }
}
```

Conventions:

- The passes module carries `#![forbid(unsafe_code)]`, which closes the raw-pointer residual of the two `Pass` bounds.
- `self` carries **nothing**: every pass is a zero-sized name token (`const`-asserted in `run_pass`), kept only so call sites read as `run_pass(OptimizeChunksPass, ..)`. Configuration is runtime data and enters through `InputRead` like everything else. Caveat: the assert is evaluated at monomorphization — `cargo build` rejects a stateful pass, but `cargo check` and rust-analyzer do not run it.
- Slot types: empty = `()`; a single artifact = the bare type; multiple = a named per-pass struct (`XxxPassInputRead { .. }`, derives `Copy`), which doubles as the pass's greppable dependency manifest.
- Impl signatures copy the trait verbatim: write `read: Self::InputRead<'_>` even when the slot is `()` — spelling the parameter as a concrete type drops the method's lifetime binder and fails E0195.
- Naming: a pass type ends in `Pass` (`OptimizeChunksPass`); its input struct is `<PassName>InputRead`. Since passes are zero-sized name tokens, the suffix keeps call sites unambiguous — and `rg 'struct \w+Pass;'` is the complete pass inventory (it matches unit structs only; a fielded `struct XPass(u64);` evades the grep, and it is the build-time assert that actually rejects it).
- **The hard guarantee is `Sealed<T>`, and the harness applies it**: `run_pass` wraps every read-side output, so nothing can exit `OutputRead` unfrozen — by construction, not convention. `Sealed<T>` has no mutation path even for an owner, so frozenness survives re-ownership with no ledger discipline needed.
- **Representation changes are the soft layer**: inside artifacts, prefer `Vec<T> → Box<[T]>`, `String → Box<str>`, `IndexVec<I, T> → IndexBox<I, [T]>`, maps → sorted boxed slices — dropped capacity, fixed lengths (indices cannot dangle by growth or removal), honest types even when viewed without the wrapper. Hygiene and defense-in-depth; correctness does not rest on it. Draft/final type pairs stay where sealing does real work (compaction); `Sealed<T>` alone suffices where it does not.
- `Arc<T>` is **not** a seal: a unique holder melts it with `Arc::get_mut` / `Arc::try_unwrap` — frozenness would hinge on the runtime reference count, not the type. `Arc` is a sharing mechanism: compose it as `Arc<Sealed<T>>` when a sealed artifact must be shared.
- `PassCtx` is the single sanctioned `&mut`: write-only sinks (diagnostics now, devtools trace later). It never contains pipeline data; passes may write it but never read it — and make that constructional, not reviewed: write methods take `&mut self`, read/drain methods take `self` by value, so a pass holding only `&mut PassCtx` cannot call them. `run_pass` (the wrapper) owns tracing spans and stamps every diagnostic with the emitting pass's name. One implementation note: `async fn` in a `pub` trait trips the `async_fn_in_trait` lint (callers cannot add `Send` bounds); keep the trait `pub(crate)` or record an explicit `#[allow]`.
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
  type OutputRead    = ChunkGraph;        // run_pass hands the driver a Sealed<ChunkGraph>
  type OutputOwned   = ();
  // ...
}

// 3) Derive: the most common shape — owns nothing, reads sealed data, mints a new sealed artifact.
#[derive(Clone, Copy)]
pub struct AssignNamesPass;

impl Pass for AssignNamesPass {
  type InputRead<'a> = AssignNamesPassInputRead<'a>;   // { graph: &'a ChunkGraph, options: &'a NormalizedBundlerOptions }
  type InputOwned    = ();
  type OutputRead    = ChunkNames;        // e.g. IndexBox<ChunkIdx, [ArcStr]>; arrives as Sealed<ChunkNames>
  type OutputOwned   = ();
  // ...
}
```

The driver is a typed `let`-chain (deliberately **not** `Vec<Box<dyn Pass>>` — heterogeneous signatures are the point):

```rust
let (_, graph) = run_pass(OptimizeChunksPass, &mut cx, optimize_reads, graph).await?;
let (graph, ()) = run_pass(SealChunkGraphPass, &mut cx, (), graph).await?; // graph: Sealed<ChunkGraph>
let (names, ()) = run_pass(AssignNamesPass, &mut cx, names_reads, ()).await?;
```

`OutputRead = ()` still comes back as `Sealed<()>` — discard it with `_`; a literal `()` pattern does not match it (E0308). The owned side is never wrapped, so `()` patterns are fine there.

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
// error[E0599]: no method named `add_chunk` found for struct `Sealed<ChunkGraph>`
// (method lookup walks through Deref and still finds nothing — mutators exist
//  only on DraftChunkGraph, and what you hold after sealing is Sealed<ChunkGraph>)
```

### Parallelism: signatures expose the candidates, the compiler checks the join

Two passes are concurrency candidates exactly when their `InputOwned`s are disjoint and neither reads the other's output — both facts sit in the slot types. What each layer actually guarantees:

- **Signatures** expose the candidates: disjoint owned data, no artifact dependency between the two.
- **The borrow checker (plus `Send`/`Sync`)** proves the join is free of data races — an unsound join (shared owned data, missing artifact) fails to compile.
- **Semantic independence is not proven; it is a stated discipline**: no interior mutability in pipeline data, no globals, no order-dependent external calls (plugin hooks, I/O) inside candidate passes, and effects only through per-branch `PassCtx` sinks that the driver merges in a fixed order. Under those rules — and only under them — a compiling join is also deterministic.

```rust
// ComputeLinksPass : InputRead = (&ChunkGraph, &SymbolRefDb), InputOwned = ()
// AssignNamesPass  : InputRead = (&ChunkGraph, &Options),     InputOwned = ()
let ((links, ()), (names, ())) = try_join!(
  run_pass(ComputeLinksPass, &mut cx_a, (&graph, &symbols), ()),
  run_pass(AssignNamesPass,  &mut cx_b, (&graph, &options), ()),
)?;
```

Realism note: `try_join!` interleaves the two futures within one task — it buys overlap, not multicore speedup for CPU-bound passes, and futures borrowing `&graph` are not `'static`, so they cannot be `tokio::spawn`ed. Real multicore parallelism lives inside passes (rayon) unless the driver grows a scoped-task story.

### Other benefits

The dependency graph is greppable — impact analysis without reading bodies:

```console
$ rg 'symbol_db: &' -g '*.rs'           # every pass that reads the symbol table
$ rg 'InputOwned = DraftChunkGraph'     # every pass that ever owns the graph (rustfmt collapses aligned spaces)
```

Each pass is unit-testable by construction: its `InputRead` type is the exact, minimal fixture spec — no need to build whole stage outputs to test one pass.

Uniform machinery: `run_pass` is the single home for tracing spans and diagnostics provenance (`type_name::<P>()`), so observability never needs per-pass wiring.

## Enforcement

The goal is **not** to make illegal states unrepresentable — it is to make them impossible to write _quietly_. State has exactly three legal homes: locals inside `run` (unrestricted); driver-built values lent through `InputRead`; artifacts moving through the owned slots. There is no fourth place — a pass type itself is zero-sized, so "pass-internal state" is not a category that exists. What therefore has no home is state that crosses passes without appearing in any signature. The gates below force exactly that case into the open, where review can catch it — under a shared `&mut` world it was invisible by construction.

What the compiler pins:

- reads cannot be `&mut` — `InputRead<'a>: Copy` (`&mut` is never `Copy`)
- a pass value cannot carry anything at all — `run_pass` `const`-asserts `size_of::<P>() == 0`; `Pass: Copy + 'static` additionally rules out lifetime-carrying zero-sized tricks
- no escape through raw pointers — `#![forbid(unsafe_code)]` on the passes module
- pass order — `let`-chain scoping; sealing — applied by `run_pass` itself (`Sealed<T>`: no `DerefMut`, no unwrap); ownership transfer — moves

What stays review-held (the honest list):

- frozen representations inside artifacts (`Box<[T]>` over `Vec<T>`) — hygiene, not correctness; the hard freeze is `Sealed<T>`, applied by the harness
- no interior mutability in pipeline data types; no global statics holding pipeline data (the one remaining way to smuggle state past the bounds)
- artifact granularity (own exactly what you reshape); new `PassCtx` methods staying write-only (drain methods take `self` by value, unreachable through `&mut`)

Why a trait at all: uniform pass style — every pass declares the same four slots and is invoked through one wrapper — plus the shape gates above, which plain functions cannot carry (nothing stops an extra `&mut` parameter on a plain function except review). The cost — GATs and `()`/tuple ceremony — is accepted deliberately.

## Future directions

- Driver-level joins of parallel-candidate passes, once profiling shows a win worth taking (see the realism note in Why — plain `try_join!` only interleaves; multicore needs a scoped-task driver or stays inside passes via rayon).
- When the first flow adopts the contract, pin the compile-error claims (E0308 / E0425 / E0599 / E0277) as `trybuild` compile-fail tests — the doc promises compiler behavior, and tests should hold it.
- Incremental cache friendliness: explicit inputs are natural dependency keys, sealed artifacts are natural snapshot/hash units, and a pass is a natural recompute unit. To be honest: the contract was **not** designed with incremental builds as a premise, and nothing in it depends on that — it simply does not stand in the way.

## Related

- `implementation.md` — none yet; added by the first flow that adopts the contract.
