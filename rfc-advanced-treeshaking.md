# RFC: Advanced tree shaking

- Feature Name: `advanced_treeshaking`
- Start Date: 2026-08-10
- RFC PR: TBD
- Tracking Issue: TBD. The gap table below lists the 8 open issues this RFC would close.

Code references point at `rolldown@79cd87fe8`. The quoted bundler output is real output from
`rolldown@1.2.0` and `rollup@4.62.4`, reformatted to this document's style. Rollup source
references point at the `rollup` submodule at `ddc4ffab`, which is the same 4.62.4 the builds
used.

## Table of contents

- [Summary](#summary)
- [Motivation](#motivation)
- [Guide-level explanation](#guide-level-explanation)
- [Reference-level explanation](#reference-level-explanation)
- [Drawbacks](#drawbacks)
- [Rationale and alternatives](#rationale-and-alternatives)
- [Prior art](#prior-art)
- [Unresolved questions](#unresolved-questions)
- [Future work](#future-work)

## Summary

This RFC contains two main parts:

- Unlock the ability to answer tree-shaking questions about a path, such as `obj.a.b`, rather
  than about a whole top-level statement.
- The option `experimental.advancedTreeshaking` is built on that ability. It closes 13 gaps
  measured against `rollup@4.62.4`.

## Motivation

### A whole statement survives one read

A bundler can decide per path or per statement. For the given input:

```js
// mod.js
export const obj = { used: 1, DEAD_MEMBER: () => 'x', alsoDead: [1, 2, 3] };

// entry.js
import { obj } from './mod.js';
console.log(obj.used);
```

The path approach, as rollup 4.62.4 emits it:

```js
const obj = { used: 1 };
console.log(obj.used);
```

The statement approach, as rolldown 1.2.0 emits it:

```js
console.log({ used: 1, DEAD_MEMBER: () => 'x', alsoDead: [1, 2, 3] }.used);
```

The outputs differ. Each approach optimizes for something else:

- Rollup interprets the program abstractly, keeps a fact per path, and pays build time for it.
- esbuild answers one question per top-level symbol and folds locally afterwards. Rolldown
  inherited that model, and it is a large part of why rolldown is fast.

This RFC aims to provide rollup's precision, while keeping rolldown's build model intact for
users who do not ask for it.

Rolldown reached the right answer for `obj`. The module reads it, so the declaration stays. It
has no way to record that the module reads `obj.used` and nothing else.

The unit is the statement, not the value inside it. `StmtInfo` holds the top-level symbols a
statement declares (`crates/rolldown_common/src/types/stmt_info.rs`, where a comment records
"currently, we only store top level symbols"). The include pass walks from the entries and
marks statements (`crates/rolldown/src/stages/link_stage/tree_shaking/include_statements.rs`).
Rolldown emits a marked statement whole, so the whole object survives.

Rolldown does remove code inside a statement, but only later and only within a chunk. oxc folds
locally during minification, and that pass runs after chunking.

The cost in the user's units is retained bytes. This is not an issue for a small module. The cost
scales with the size of the object a library exports. `#9011` reports 1 MB of retained code
from this exact shape, through `@arcgis/core`.

<details>
<summary>What the issue tracker says</summary>

I searched 332 rolldown issues, open and closed, for tree-shaking reports. Eight are open:
[#3755](https://github.com/rolldown/rolldown/issues/3755),
[#5420](https://github.com/rolldown/rolldown/issues/5420),
[#5872](https://github.com/rolldown/rolldown/issues/5872),
[#6945](https://github.com/rolldown/rolldown/issues/6945),
[#7685](https://github.com/rolldown/rolldown/issues/7685),
[#8133](https://github.com/rolldown/rolldown/issues/8133),
[#8582](https://github.com/rolldown/rolldown/issues/8582),
[#9698](https://github.com/rolldown/rolldown/issues/9698).

**A closed issue does not mean a closed gap.** `#9011`, `#4544`, and `#7682` are closed as
duplicates, and `#6119` is closed as not planned. I built all four shapes against
`rolldown@1.2.0`, and the behaviour is still present.

</details>

### How rolldown decides what to keep

Two rules produce every retained byte above:

1. The unit of tree shaking is the top-level statement. A statement is kept or dropped whole.
2. Anything finer runs after chunking, inside the minifier, so it can never cross a chunk
   boundary.

**1.** is the direct cause. `StmtInfo` records the top-level symbols a statement declares and
nothing below them, so the finest question rolldown can ask is "does anything reference this
top-level symbol?". Everything written inside the statement stays or goes with it: an object's
members, a nested path, a call's arguments.

**2.** is what stops the later passes from repairing it. It also sets a hard ceiling that
[W3](#w-the-write-set-and-the-fixpoint) measures, and it is the reason this RFC puts the
analysis in the link stage.

### The dilemma

> 1. The unit of tree shaking is the top-level statement. A statement is kept or dropped whole.
> 2. Anything finer runs after chunking, inside the minifier.

But what if we break rule `1`? What will we get?

Rolldown has one option today that looks like the answer. `propertyReadSideEffects: false` tells
the bundler that reading a property does nothing, so an unused read can go:

```js
const o = {
  get a() {
    console.log('REAL_GETTER_EFFECT');
    return 1;
  },
  b: 2,
};
o.a; // the getter runs, so this log must happen
export const keep = 1;
```

Rolldown at default settings keeps all of it, which is correct and large:

```js
const o = {
  get a() {
    console.log('REAL_GETTER_EFFECT');
    return 1;
  },
  b: 2,
}; // logs REAL_GETTER_EFFECT, expected: logs REAL_GETTER_EFFECT
o.a;
const keep = 1;
export { keep };
```

Rolldown with `propertyReadSideEffects: false` is smaller and wrong:

```js
const keep = 1; // logs nothing, expected: logs REAL_GETTER_EFFECT
export { keep };
```

Rollup at default settings is correct and smaller. It keeps the getter and the read, and it
removes the unread `b: 2`. So the heap model still applies when an accessor is present, and "bail
on any object with a getter" is not a shortcut.

The behaviour changed. **This is just one case.** It shows that an assumption cannot stand in for
the analysis.

The reason is that the current model mixes up two things:

- **Reachability:** does anything reference this top-level symbol?
- **Use:** which parts of the value does anything read, and does touching them do anything?

Rolldown answers the first question and uses that answer for the second. The two are
independent. A program can reference `obj` and read one member of it. A program can read a
property that does nothing and a property that logs. Rule `1` cannot tell those apart, so it
gives both the same answer, and the only dial on offer flips the assumption for the whole build.

So both options today are bad. Rule `1` keeps code that nothing reads. A global assumption
changes what programs do.

### Thirteen measured gaps

I distilled each tree-shaking report into a case and built it with both bundlers. Thirteen
shapes retain code in rolldown that rollup removes.

| #   | gap                                                | dimension | issues                                                                                                               |
| --- | -------------------------------------------------- | --------- | -------------------------------------------------------------------------------------------------------------------- |
| H1  | object literal keeps unread members                | heap      | [#9011](https://github.com/rolldown/rolldown/issues/9011), [#4544](https://github.com/rolldown/rolldown/issues/4544) |
| H2  | nested property path keeps unread leaves           | heap      | [#9011](https://github.com/rolldown/rolldown/issues/9011)                                                            |
| H3  | `'x' in ns` materializes the whole namespace       | heap      | [#5420](https://github.com/rolldown/rolldown/issues/5420)                                                            |
| E1  | unused property read keeps the object              | effect    | [#5872](https://github.com/rolldown/rolldown/issues/5872)                                                            |
| E2  | getter with an effect has no model                 | effect    | [#5872](https://github.com/rolldown/rolldown/issues/5872)                                                            |
| E3  | unused object spread survives                      | effect    | [#8582](https://github.com/rolldown/rolldown/issues/8582)                                                            |
| E4  | assignment to a dead binding keeps the value alive | effect    | [#7685](https://github.com/rolldown/rolldown/issues/7685), [#3755](https://github.com/rolldown/rolldown/issues/3755) |
| T1  | constant return value not folded at the call site  | transfer  | [#6119](https://github.com/rolldown/rolldown/issues/6119)                                                            |
| T2  | return value through a property read not folded    | transfer  | [#6119](https://github.com/rolldown/rolldown/issues/6119)                                                            |
| T3  | unused call argument survives                      | transfer  | [#8133](https://github.com/rolldown/rolldown/issues/8133)                                                            |
| W1  | function whose body writes only a dead flag        | fixpoint  | [#7682](https://github.com/rolldown/rolldown/issues/7682)                                                            |
| W2  | mutually guarded flags                             | fixpoint  | [#6945](https://github.com/rolldown/rolldown/issues/6945)                                                            |
| W3  | never-written binding across a chunk boundary      | fixpoint  | [#9698](https://github.com/rolldown/rolldown/issues/9698)                                                            |

### The gaps are not independent

This is the reason to treat the 13 as one change and not as 13 tickets.

```js
const config = { mode: 'prod', DEAD_CFG_MEMBER: 1 };
let mode = config.mode; // step 1 — needs H, a path read
function setMode(m) {
  mode = m;
} // never called — step 2, needs W
function isProd() {
  return mode === 'prod';
} // step 3 — needs T
function devOnly() {
  console.log('DEAD_DEV');
} // step 5 — dead only after step 4
export function main() {
  if (!isProd()) {
    devOnly();
  } // step 4 — prune
  return 1;
}
```

| build                     | result                                                      |
| ------------------------- | ----------------------------------------------------------- |
| rollup                    | `function main() { return 1; }` — the whole chain collapses |
| rolldown, `minify: false` | keeps `DEAD_DEV`, `DEAD_CFG_MEMBER`, `isProd`               |
| rolldown, `minify: true`  | still keeps `DEAD_DEV`, `DEAD_CFG_MEMBER`                   |

The chain crosses three dimensions in five steps. Nothing folds `config.mode`, so `mode` is
never known, so `isProd()` never folds, so the branch survives, so `devOnly` stays reachable.
One missing dimension stops the other three from firing.

## Guide-level explanation

### The solution

This RFC introduces:

1. A path-keyed analysis in the link stage, which keeps a fact per path instead of a bit per
   statement.
2. A feature `experimental.advancedTreeshaking`, built on top of that analysis, which turns it
   on for a build.

### What's a path-keyed analysis?

A path-keyed analysis is nothing but a set of facts that let you:

1. Ask what a path holds, such as "what is `obj.a.b`?".
2. Ask whether touching that path does anything, such as "does reading `obj.a.b` run a getter?".

The basic shape is six questions, asked of every node in the program:

```ts
// the whole domain, as questions about a path
interface Entity {
  getLiteralValueAtPath(path); // what is obj.a.b?
  getReturnExpressionWhenCalledAtPath(path); // what does obj.f() return?
  hasEffectsOnInteractionAtPath(path, kind); // does reading, writing, or calling it do anything?
  deoptimizePath(path); // give up on obj.a.b, forever
  includePath(path); // the program reads obj.a.b, so emit it
  includeCallArguments(call); // this call needs its arguments
}
```

The domain is small, and the ability it unlocks is large. Complexity mainly comes from making the
four dimensions agree with each other, and from keeping the fixpoint cheap enough to run on every
build.

For the example in [the example above](#a-whole-statement-survives-one-read), we now get this:

```js
const obj = { used: 1 };
console.log(obj.used);
```

### The four dimensions

The analysis answers questions about a path, such as `obj.a.b`, rather than about a symbol.
Four parts carry it. Rollup's names appear here because the
[Reference-level explanation](#reference-level-explanation) maps each gap onto them.

| dimension                    | question it answers                                              | gaps  |
| ---------------------------- | ---------------------------------------------------------------- | ----- |
| **H** heap model             | which paths of this object does the program read?                | H1–H3 |
| **E** effect domain          | does touching this path do anything observable?                  | E1–E4 |
| **T** transfer               | what does this call return, and what does this parameter hold?   | T1–T3 |
| **W** write set and fixpoint | does anything write this binding, and what follows once we know? | W1–W3 |

Two properties matter more than the list.

**Value and effect are separate domains.** What `o.b` holds and whether reading `o.b` runs code
are different questions about the same path. The analysis answers both.

**The four run as one fixpoint.** A result from any dimension can be the input to any other, as
[the chain above](#the-gaps-are-not-independent) shows. They iterate until nothing changes.

### What's `advancedTreeshaking`?

Previously, we identified the code that rolldown keeps and rollup does not:

- object properties that nothing reads;
- statements whose only work is a property read with no effect;
- branches whose condition folds through a call or a never-written binding;
- functions and arguments that only the removed code referenced.

The option turns the analysis on for a build:

```ts
experimental: {
  advancedTreeshaking?: boolean; // Defaults to `false`, which means today's statement-level tree shaking
}
```

One boolean, not a set of sub-flags per dimension. The chain above is the reason. A build with H
and W but not T gets almost none of the benefit, so exposing that combination would offer users a
choice with no good answers. The dimensions land incrementally in development. They reach users
together.

> [!NOTE]
> Once the analysis exists, `treeshake.propertyReadSideEffects` accepts `true` and means "ask
> the object", which is rollup's default and what `#5872` requests. Today rolldown accepts only
> `false` and `'always'`, the two assumptions, with no analyzing mode between them.

A flag is warranted because of how this fails. A bug in the analysis does not break the build. It
emits a bundle that runs and is quietly wrong, in code the user did not write. That deserves an
off switch while the feature is experimental, and something to bisect against.

### How code is selected

The algorithm looks like this:

```js
markEntriesIncluded();

do {
  needsAnotherPass = false;
  for (const module of modules) {
    for (const statement of module.statements) {
      // Each visit may call includePath(path) on some entity, which records a new
      // fact and sets needsAnotherPass = true.
      statement.include();
    }
  }
} while (needsAnotherPass);

emitOnlyIncludedPaths();
```

> [!NOTE]
> This loop runs before chunk assignment. [W3](#w-the-write-set-and-the-fixpoint) is the case
> that forces that placement.

### What does not change

- **Output formats.** No new format, no new runtime helper.
- **Correct programs.** Removing code that nothing reads is invisible, and
  [Preserved semantics](#preserved-semantics) records the cases that bound this.
- **Existing options.** `propertyReadSideEffects: false` keeps its current meaning. It is still
  an assumption, and it is still unsafe.

## Reference-level explanation

### Naming things

Three terms carry the rest of this document. [The four dimensions](#the-four-dimensions)
defines **H**, **E**, **T**, and **W**.

A `path` is a sequence of property keys reached from a binding, such as `[a, b]` for `obj.a.b`.
The empty path is the binding itself. Rollup keys every fact by a path
(`rollup/src/ast/utils/PathTracker.ts`).

`Deoptimization`, also called `widening`, is the analysis giving up on a path and answering
"unknown" for it from then on. `UnknownKey` is the top of the path lattice, and rollup's
`hasLostTrack` flag never clears once set. Giving up is what makes the analysis terminate.

The `fixpoint` is the loop that re-runs the whole analysis until no new fact appears. The four
dimensions run inside one fixpoint, not four.

Each dimension below names the rollup mechanism, then the gaps that need it. All output shown is
real output from the two bundlers.

### H. The heap model: facts keyed by path

Rollup builds an `ObjectEntity` (`rollup/src/ast/nodes/shared/ObjectEntity.ts`) for an object
literal and calls `includePath` for each path the program reads. It emits only the properties on
an included path. Rollup keys every path through `ObjectPath`
(`rollup/src/ast/utils/PathTracker.ts`), where `UnknownKey` is the top of the lattice.

**H1** is the example in [the example above](#a-whole-statement-survives-one-read).

**H2 — the model must be recursive.**

```js
export const deep = { a: { b: { c: 1, DEAD_DEEP: 2 } } };
console.log(deep.a.b.c);
```

Rolldown keeps `DEAD_DEEP`. Rollup emits `const deep = { a: { b: { c: 1} } };`.

**H3 — `in` forces the whole namespace.**

```js
// mod.js
export const a = 1;
export const DEAD_NS_MEMBER = 'dead';
// entry.js
import * as ns from './mod.js';
if ('a' in ns) console.log(ns.a);
```

Rollup emits `const a = 1; console.log(a);`. It resolves the `in` test against the namespace's
known shape and never builds the object. Rolldown emits the `__defProp` and `__exportAll`
helpers, builds the namespace, and keeps `DEAD_NS_MEMBER`. Libraries use this shape to probe a
peer dependency for a newer export, which is what `#5420` reports.

### E. The effect domain: effects keyed by path

The question here is not what a value is. It is whether touching it does something. Rollup
answers with `hasEffectsOnInteractionAtPath`, keyed by the same paths, and discriminated by
interaction: accessed, assigned, or called.

Rollup needs no option for this. Its default is
[`propertyReadSideEffects: true`](https://rollupjs.org/configuration-options/#treeshake-propertyreadsideeffects),
and `true` does not mean "assume a read has effects". `hasAccessEffect`
(`rollup/src/ast/nodes/MemberExpression.ts:566`) delegates to
`this.object.hasEffectsOnInteractionAtPath(…)`, so rollup asks the object. Only `'always'` assumes
without asking. I built E1, E2, and E3 with `propertyReadSideEffects: true` set explicitly, and
rollup still removed all three.

**E1 — an unused property read.** `API.USERS;` as a statement. Rolldown keeps the object and the
read. Rollup emits an empty module, at default settings.

**E2 — a getter that the program does not read.**

```js
const o = {
  get a() {
    console.log('GETTER_EFFECT');
    return 1;
  },
  b: 2,
};
o.b; // reads b, not a
```

Rollup separates `gettersByKey` from `propertiesAndGettersByKey`, so it knows a read of `b`
cannot run the getter on `a`. It removes the statement. Rolldown keeps everything.

**E3 — an unused object spread.** A spread reads every property, so it is an effect question.
With no getters present the reads do nothing, and the unused result is dead.

```js
const Proto = {
  [TypeId]: TypeId,
  pipe() {
    return 1;
  },
  toJSON() {
    return 2;
  },
};
({ ...Proto });
({ ...Proto });
```

Rollup removes all of it. Rolldown keeps `Proto` and both spreads. This is the shape in `#8582`,
from `effect` v4.

**E4 — an assignment to a dead binding.** Rollup treats a write to a binding that nothing reads
as having no effect. The `INTERACTION_ASSIGNED` branch of
`LocalVariable.hasEffectsOnInteractionAtPath` reads
`if (this.included) return true; if (path.length === 0) return false;`.

```js
function _child(node) {
  return node.firstChild;
}
function _txt(node) {
  return node.textContent;
}
const child = (...args) => child.impl(...args);
child.impl = _child;
const txt = (...args) => txt.impl(...args);
txt.impl = _txt; // txt is never used
export function main() {
  return child(document.body);
}
```

Rollup keeps `_child` and `child`. Rolldown keeps `_txt` and `txt` too, because `txt.impl = _txt`
reads as a use of `_txt`. This is the second half of `#3755` and the whole of `#7685`.

### T. Interprocedural transfer: across a call edge

A call is an edge in the analysis. Rollup carries facts across it in both directions.

**T1 — the return value flows back.** `getReturnExpressionWhenCalledAtPath` gives the call site
the callee's return.

```js
function getFlag() {
  return false;
}
const flag = getFlag();
export function main() {
  if (flag) console.log('DEAD_RETURN');
  return 1;
}
```

Rollup emits `function main() { return 1; }`. Rolldown keeps the function, the binding, and the
branch.

**T2 — the return value may be a path.**

```js
const cfg = { debug: false };
function isDebug() {
  return cfg.debug;
}
export function main() {
  if (isDebug()) console.log('DEAD_CFG');
  return 1;
}
```

Same outcome, and it needs dimension H as well. T2 is the clearest single case for why the
dimensions cannot be built independently.

**T3 — the argument flows into the parameter.** Rollup removes `clientHello` below and narrows the
call to `pick(serverHello)`, through `includeCallArguments` and `ParameterVariable`.

```js
const isServer = true;
function serverHello() {
  console.log('server');
}
function clientHello() {
  console.log('client');
}
function pick(s, c) {
  return isServer ? s : c;
}
export const hello = pick(serverHello, clientHello);
```

Rolldown keeps `clientHello` and both parameters. Reported as `#8133`.

### W. The write set and the fixpoint

`LocalVariable.isReassigned` (`rollup/src/ast/variables/LocalVariable.ts:125`) records whether
anything writes a binding. When nothing writes it, every read resolves to the initializer. The
`do { … } while (needsTreeshakingPass)` loop at `rollup/src/Graph.ts:174` re-runs the analysis
until results stop changing.

**W1 — a function that writes only a dead flag.** Rollup emits `createApp` alone; rolldown keeps
all three declarations. Rolldown's minifier closes this one when the whole program lands in one
chunk.

```js
let initialized = false;
function initFeatureFlags() {
  if (initialized) return;
  initialized = true;
}
export function createApp() {
  initFeatureFlags();
  return { mount() {} };
}
```

**W2 — the fixpoint specifically.** A single pass gets none of these three.

```js
let hydrating = false,
  mismatch = false;
function set_hydrating(h) {
  hydrating = h;
}
if (hydrating) {
  mismatch = true;
}
if (mismatch) {
  set_hydrating(true);
}
if (hydrating) {
  console.log('DEAD_HYDRATE');
}
```

Proving the first `if` dead is what proves the second dead, which is what proves the third dead.
Rollup removes all of it. Rolldown keeps all of it.

**W3 — the analysis must precede chunking.** This case decides where the analysis lives.

```js
// flag.js
export let custom_render = false;
export function enable_custom_render() {
  custom_render = true;
}
// main.js
import { custom_render } from './flag.js';
import { mount as mc } from './client.js';
export function mount(...a) {
  if (custom_render) {
    return mc(...a);
  }
  throw new Error('server only');
}
// another.js
import { custom_render } from './flag.js';
console.log(custom_render);
```

Nothing reachable calls `enable_custom_render`, so `custom_render` stays `false`.

| entries                    | rolldown                              | rollup                     |
| -------------------------- | ------------------------------------- | -------------------------- |
| `main.js` only             | folds the branch, removes `client.js` | folds, removes `client.js` |
| `main.js` and `another.js` | **keeps the branch and `client.js`**  | folds, removes `client.js` |

The second entry moves `flag.js` into a shared chunk. No chunk then holds both the binding and
the branch. Rollup still folds, because it ran the analysis on the whole module graph before
chunking. This is `#9698`.

### Preserved semantics

Crossing into path granularity must preserve rolldown's existing observable semantics:

- **Evaluation order and side effects.** Removing a read that has no effect cannot move one that
  does.
- **Accessor semantics.** A getter that the program reads must still run. See
  [the dilemma](#the-dilemma) for the case that bounds this.
- **Live bindings.** A later write to an exported binding stays visible to importers.
- **Existing overrides.** `moduleSideEffects`, `annotations`, and `manualPureFunctions` keep
  their current meanings and still win.

A property that a plugin adds after the analysis runs is outside this contract, so the analysis
reads the module graph as the link stage sees it and does not attempt to predict later
transforms.

### Claims excluded after measurement

Seven reports look like gaps and are not. I built each one. Recording them keeps the scope honest
and stops them from being re-filed.

| claim                                                     | issue                                                       | result                                                                                                                    |
| --------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `@__NO_SIDE_EFFECTS__` lost on a non-function initializer | [#9943](https://github.com/rolldown/rolldown/issues/9943)   | **rolldown better** — it removes `math` and marks the call pure; rollup keeps `math`                                      |
| `moduleSideEffects` ignored after cross-module folding    | [#8195](https://github.com/rolldown/rolldown/issues/8195)   | parity, with one entry and with two                                                                                       |
| `export *` output not tree-shake friendly                 | [#7874](https://github.com/rolldown/rolldown/issues/7874)   | parity — rolldown emitted `console.log(1)`, rollup kept the binding                                                       |
| side-effect-free dynamic import retained                  | [#3918](https://github.com/rolldown/rolldown/issues/3918)   | parity — neither removes it                                                                                               |
| no pure annotation for `await`                            | [#8940](https://github.com/rolldown/rolldown/issues/8940)   | rollup does not support this either ([rollup#5187](https://github.com/rollup/rollup/issues/5187))                         |
| `require('./foo.json')` not shaken like `import`          | [#8197](https://github.com/rolldown/rolldown/issues/8197)   | not a rollup comparison — rollup core has no CommonJS support                                                             |
| the CommonJS wrapper blocks later passes                  | [#10483](https://github.com/rolldown/rolldown/issues/10483) | rollup is not better — `@rollup/plugin-commonjs` also emits a lazy memoized wrapper. Webpack 5.109 is the one that hoists |

Rolldown also matches or beats rollup on cross-module constant folding into a branch, unused
pure-call results, a const read from several expressions, a `var`-declared flag, and namespace
property reads. Neither bundler removes an unused class method, an unused static class field, or
reasons through a function that mutates its argument.

My repros for `#8195` and `#7874` did not reproduce the reported gap. That is evidence about my
repro, not proof the issues are invalid. Both describe larger graphs than I built.

## Drawbacks

Three costs arrive with the code. An analysis bug that emits a quiet, wrong bundle is not a
fourth. That is a failure mode, not a design cost, and [the option](#whats-advancedtreeshaking)
exists to bound it. Snapshot churn at the default flip is not one either. That is a one-time
review cost, and the option only delays it.

### The benefit is not linear in the work

[The chain in Motivation](#the-gaps-are-not-independent) shows why. A half-finished analysis buys
close to nothing, so this change cannot be de-risked by shipping a third of it. That is unusual,
and it makes the estimate harder to stage.

### Build time

Rollup pays for this output in build speed, and rolldown would pay something similar for the same
result. The analysis is a whole-graph fixpoint, so it cannot be parallelized per chunk the way
minification can. Speed is rolldown's main claim, so this cost lands on the product's strongest
axis. The option bounds the exposure to builds that ask for it, and it cannot reduce the cost for
those builds.

### Memory

Path-keyed facts are more state than one bit per statement, and the deoptimization trackers hold
entries per path per entity.

## Rationale and alternatives

[Why this shape](#why-this-shape) is the rationale. The next two sections are the alternatives
that would give the same result, each with the reason against it. The last section says what this
RFC does not change.

### Why this shape

**It matches a design with a decade of production evidence.** Rollup's engine is not a research
prototype. `ExpressionEntity` (`rollup/src/ast/nodes/shared/Expression.ts:36`) declares ten
methods, six of which carry the domain, and every node implements them. The shape is known to
terminate and to be maintainable by a small team.

**It reuses rolldown's existing analysis rather than replacing it.** `constant_export_map`
already folds cross-module constants, and oxc folds locally. Those keep working. This RFC adds
dimensions to the domain, and it does not discard the two-point value lattice already there.

**Its failure mode is bounded by an option.** With `advancedTreeshaking` off, no existing
behaviour moves.

This is an extension of rolldown's current tree shaking, not a replacement with another model.

### Do it in the minifier

Measured, this alternative looks attractive: rolldown's minifier already closes W1 and partly
closes E4 and T3. Nothing comes for free here — it fails for two reasons.

First, a subset does not compose. [The chain in Motivation](#the-gaps-are-not-independent) stalls
at its first missing step, and minification does not rescue it.

Second, the fixpoint must finish before chunking. Combine two facts. The analysis is one
fixpoint, so a result from any dimension can feed any other. And some results must exist before
chunking, or the chunk boundary destroys them, which W3 proves. Therefore the whole fixpoint must
run before chunking.

Any part left downstream is a part whose output can never re-enter the loop.
It cannot break rule `2`. So it either stays behind the chunk boundary, or it stops being a
minifier pass.

Two further limits apply to a minifier whatever the placement. It cannot read non-syntactic
metadata such as `package.json` `sideEffects` or plugin facts. And it cannot recover what the
bundler already obscured, which H3 shows: once rolldown lowers a namespace into `__exportAll`
calls, the shape the analysis needs is gone.

What remains in the minifier is what does not change module-level reachability: identifier
mangling, `!0` for `true`, and peephole rewrites inside a function body.

### Port rollup's engine as it stands

Rollup's precision costs build time, and a direct port carries that cost across. Two properties
are worth copying, and one policy is worth revisiting.

Copy the interface and the path keying. Both are load-bearing and neither is expensive by itself.

Revisit the widening policy. Rollup gives up rather than staying precise: `deoptimizePath` widens
to top in one direction, and `hasLostTrack` never clears. That is what keeps it tractable, and it
is also the dial that trades output size against build time. Rolldown should set that dial for
itself rather than inherit rollup's setting.

One measurement supports revisiting it. Rollup's interpreter is narrower than its own types
suggest. I built a function returning `{}` — an unknown value with known truthiness — and rollup
did **not** fold `if (!v)`. `UnknownTruthyValue` comes only from `ConditionalExpression` and
`LogicalExpression`. So the bar to clear is a specific, incomplete engine, not a textbook one.

### Relationship to the existing folding passes

- This proposal does not require any change to `constant_export_map` or to `inlineConst`. They
  fold cross-module constants today and keep doing so.
- It does not remove work from oxc either. Peephole rewrites and mangling stay where they are.
- We prefer to let the link-stage analysis feed both later passes rather than duplicate them, but
  they are not strictly connected. [Unresolved questions](#how-does-it-interact-with-oxc) leaves
  the split open.

## Prior art

### rollup

The reference implementation, and the target this RFC measures against. Rollup's model is:

1. Interpret the program abstractly, keying every fact by a path.
2. Re-run the whole analysis until no new fact appears.

The engine is four parts: a value lattice (`nodes/shared/Expression.ts:16`), path keying
(`ast/utils/PathTracker.ts`), an effect domain (`hasEffectsOnInteractionAtPath`), and a fixpoint
(`Graph.ts:174`). It is abstract interpretation with deoptimization, which is why it terminates
on arbitrary input. [`treeshake`](https://rollupjs.org/configuration-options/#treeshake) exposes
the dials.

### esbuild

esbuild does symbol-level reachability and local constant folding, with no path-keyed heap model.
Rolldown's current position is close to esbuild's. There is no path analysis to compare against.

### webpack

[`optimization.usedExports`](https://webpack.js.org/configuration/optimization/#optimizationusedexports)
records which exports a module's importers read, so webpack keys some facts more finely than a
symbol. It stops at the export boundary and does not track nested paths inside an object. So
webpack sits between esbuild and rollup on dimension H, and it has no equivalent of dimensions T
or W.

## Unresolved questions

### Where does the analysis run?

The link stage is the constraint that W3 sets. Whether it is one pass over the module table or a
phase inside the existing link stage is open.

### What is the widening policy?

Rollup widens to top on the first unknown write and never recovers. A less eager policy finds more
and costs more. This is the main dial.

### How does it interact with oxc?

oxc already folds locally. Does the link-stage analysis feed it, replace part of it, or run beside
it? A shared answer avoids two passes proving the same fact.

### What is the build-time budget?

A number should be agreed before the work starts, because the widening policy is chosen against
it.

### When does the option flip?

Is the bar the 13 gaps closed, or a real application bundling and running unchanged? And does
`advancedTreeshaking` stay afterwards as an escape hatch?

### Is dimension T wanted at all?

`#6119` is closed as not planned. If that closure was a decision about the whole dimension, T1
and T2 leave this RFC, and T3 needs its own case.

### Which plugins assume their injected code survives?

The analysis can now remove code a plugin emits, for a reason the plugin cannot see. E4 is the
sharpest case. A write to a binding that nothing reads becomes dead, so a plugin that registers
something by assigning to an otherwise-unused object loses that write.

`moduleSideEffects`, `annotations`, and `manualPureFunctions` keep their meanings, so a plugin
that already marks its module survives. The open question is which plugins rely on today's
behaviour without marking anything.

Vite's `define` is not at risk. It replaces constants statically at build time, so more folding
is the outcome it wants.

## Future work

This RFC gives rolldown the ability to hold a fact per path. Closing the 13 gaps is one feature
built on top of it, and the ability reaches further.

### Per-path tree shaking of class members

Neither bundler removes an unused class method or static field today. A path-keyed heap model is
the prerequisite, so this becomes reachable rather than free.

### `propertyWriteSideEffects` on the same footing

Rolldown has this option and rollup does not. Once writes carry a path key, it can become an
analysis rather than an assumption, the same way `propertyReadSideEffects` does here.

### Cross-chunk facts for `preserveModules`

The analysis runs before chunking, so its results are available to chunk assignment. Whether
chunking should consult them is a separate design question.
