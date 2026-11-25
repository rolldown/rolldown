# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

## Summary

`experimental.importDefer` keeps the deferred evaluation of `import defer` and `import.defer()` in the output. The support grows in steps. Each step supports more shapes of a build, and a build with another shape falls back to an eager import. A build with the option off must give the same output as before the option existed.

## Whole-build fallback

### Goal

A build with a shape that the deferral does not support yet gives the output of the build with the option off, and a warning for each deferred import. Thus each step can add the deferral for some shapes, and all other shapes keep the eager result that they had before.

### Decisions

- The decision is for the whole build, not for each import. The conditions (no `import.defer()` of a CommonJS module, no external module that the output format evaluates before the first access, no CommonJS file that runs its module at load, the order rule of top-level `await`) are properties of the module graph and the options. A decision for each import needs rules for deferred and eager closures across chunks, cycles and async modules.
- The decision runs after the scan and before the link stage, in `scan_modules_impl`. With `experimental.incrementalBuild`, the scan sees only the changed modules, and the normalized scan output has all modules. An error (a write to a deferred namespace in a build that falls back) then fails the build as a scan error does: `buildEnd` and `closeBundle` get the error, and no link step adds more errors.
- The scanner keeps the eager results and saves the deferred results apart. Thus a fallback gives the scan results of the option-off build exactly. A second analysis at link time cannot give the same results, because the scan reads a partial `constant_export_map` that the link stage does not have.
- The conditions are conservative. For example, an import of an external module after a deferred import of a module with top-level `await` counts in each build that can make more than one chunk, also when the build makes only one chunk.
- A build falls back when a code splitting group sets `includeDependenciesRecursively: false`. Such a group can make two chunks import each other. In such a cycle, a chunk can call an init function or a runtime helper of the other chunk before that chunk runs, and the build throws a `TypeError`. The build without `import defer` has no such calls.
- A build falls back when the output file of a CommonJS module of a closure runs the module at load: with `preserveModules` and `esm` output, and for a CommonJS user entry in `cjs` output. A deferred importer imports that file, so the module runs before the first access, and the build gives no warning. The fallback gives the same order of that module, with a warning.

### Rejected alternatives

- A decision at the start of the link stage. `buildEnd` then gets no error for a write to a deferred namespace, and the build fails later than with the option off.
- An analysis of the statements at link time with the eager rules, instead of the saved scan results. The results can differ from the option-off build (see the `constant_export_map` above).
- A file for a CommonJS module that does not run it at load, in `esm` output with `preserveModules`. The file must export a `default` binding that `require_x()` assigns, so the wrapper needs a new form.

## Shared CommonJS namespaces

### Goal

The proposal gives one deferred namespace for each module. Without a merge, each `import defer` of a CommonJS module has its own `__deferNs` object and its own `__toESM` namespace, also for two imports in one file. An eager import shares one namespace for the importers with no wrapper in one chunk (`merge_cjs_namespace`). The deferred form has the same scope.

### Decisions

- The deferred records have their own groups, apart from the groups of `merge_cjs_namespace`. An eager import assigns `import_x` at the import statement, and a deferred access assigns it at the first access. One shared binding would get two `__toESM` objects in one run.
- The interop mode is part of the key. The eager merge keys by module only, so its merged binding has the mode of the first importer. A deferred access assigns `import_x` with the mode of its own importer, so a merge by module only would make the mode depend on the access that runs first.
- Only importers with no wrapper take part, as in the eager merge. A wrapped importer declares its symbols inside its wrapper, where other modules cannot read them.
- Nothing merges with `strictExecutionOrder`, as in the eager merge. The order wraps of the generate stage give the importers wrappers that `wrap_kind()` does not show. A merge then puts the declaration in the wrapper of one importer, and another importer can read it before that wrapper runs. The strict variant of the fixture `commonjs_shared_namespace` checks this rule.

### Rejected alternatives

- A declaration at the top of the chunk, which also covers the importers with a wrapper. `__deferNs` evaluates nothing, so the declaration can move there. This needs new placement code, and the eager namespace has the same gap.

## Top-level await

### Goal

A deferred import of a module with top-level `await` keeps the steps of native Node where a bundle can: the importer waits for the modules with top-level `await` first, a later access evaluates the rest synchronously, and an access to a module that is still evaluating throws a `TypeError`.

### Decisions

- Only the tracked modules get a state record (`__esmState`). The other modules of a closure keep `__esmMin`, so a closure with no top-level `await` costs nothing more.
- Preparation starts all gathered top-level `await` modules in request order before waiting for them. One sibling can then release a promise that an earlier sibling awaits (fixture `preserve_modules_concurrent_tla`).
- Across separate static requests, a wait in an importer body can prevent the loader from reaching a sibling that releases it. The same issue arises inside an eagerly started wrapper when either the module or a dependency has top-level `await`. A wrapper reached only after its async dependencies have finished can keep deferral. For builds that can split the siblings across ESM chunks, the eager fallback delegates their scheduling to the loader.
- A build falls back when a module with no wrapper has an import after a deferred import of a module that reaches a top-level `await`. In the output, the code of that import runs before the modules with top-level `await` start, but native Node starts them first. The fallback gives the eager order, in which the modules with top-level `await` also run first. A build with `strictExecutionOrder` also falls back, because the fallback runs before the link stage and does not know the wrappers.

### Rejected alternatives

- A fallback for each build with top-level `await`. The preparation and the state records give the native result for the other shapes, also when an import comes before the deferred import.

## Cycles

### Goal

A cycle with a deferred target keeps the native result: the eager part of the cycle runs at its native position, and a deferred access to a member of a cycle that did not finish throws a `TypeError`.

### Decisions

- Only the members of a cycle with a deferred target, and the modules that reach a top-level `await`, get state records. The other cycles in the closures keep `__esmMin`.
- With a kept deferral, the position of a moved init call is the start record of the module in a walk of the eager imports, not the first import in chunk order (`ensure_lazy_module_initialization_order`). With the closure wrappers, the first import in chunk order can be in a deferred module or in another module of a cycle, and its call runs late. Then a later import of the entry ran before the module (the fixtures `cycle_eager_import_order` and `deferred_importer_first_in_chunk`). The rule applies only when the build keeps a deferral, so a build without `import defer` keeps the order that it had before.

### Rejected alternatives

- The rule for each build. A build without `import defer` can also wrap modules, and its output would change. The option must not change such a build.
- Skip only the imports inside a cycle, and keep the first import in chunk order for the other modules. A later import of a module of the cycle, other than the module that the evaluation enters first, then moves its call first, and the cycle runs from that module in a different order (fixture `cycle_member_later_import`). A deferred module that comes first in chunk order also keeps its position (fixture `deferred_importer_first_in_chunk`).

## Init after the import of a lazy host

### Goal

A lazy host is the entry chunk of a deferred ES module, and its load does not run the module. An eager importer in another chunk calls `init_x()` in its body, after all imports of its chunk. Thus `import './a.js'; import './y.js'` runs `y` before `a`, and native ESM runs `a` first. With `preserveModules`, each deferred ES module has a lazy host, so this order occurs in most such builds. A common chunk that holds a deferred module also does not run it at load, so the same order occurs for it. The goal is the native order for each importer where a move of the init keeps the order of all other steps. Where the native order is not possible, the order must not be worse than the order of the build without `import defer`.

### Decisions

- The chunk of the importer runs the init right after its import of the host. In `esm` output, it imports an init chunk with no modules, and the init chunk calls `init_x()`. In `cjs` output, it calls `require_x.init_x()` after the `require()` of the host. The load of the host still runs nothing, so a deferred-only importer still runs no module at load.
- When the import of an earlier module of the importer comes after the host, the init follows that import. In `import './a1.js'; import './w.js'; import './a2.js'`, with `a1` and `a2` in one common host and `w` in another chunk, the init of `a2` follows the import of `w`. An init right after the host runs `a2` before `w`.
- The host is the lazy host or a common chunk. An init chunk calls the init function of one module. A common chunk can hold more than one deferred module, and the init functions of the other modules must stay lazy.
- The init chunk is a chunk of the chunk graph, which the plan adds before the chunks get their names. Thus plugins see it in `renderChunk` and `augmentChunkHash`, the `imports` of each importer list it, and the hashes and the minifier apply to it. Its name does not take the name of an emitted asset.
- The plan takes an importer only when each step that native ESM runs before the module also runs before the init. An earlier import that runs code in the body of the chunk stops the plan for the later hosts. The rules over-approximate: an earlier import whose module already ran can also stop the plan. A missed move keeps the order of the output without the plan, but a wrong move runs a module too early, and another module can then read a value that is not set yet.
- Only the first module of the chunk body that runs code takes part. The init runs before the body, so it also runs before each earlier module of the body.
- When the plan declines an importer, the chunk still initializes the module at the place of the plan, if code of a later static import of the chunk runs at load. The build without `import defer` runs the module when the chunk loads the chunk of the module, so before that code. This init keeps that order. It does not give the native order: an earlier import that runs in the body then runs after the module.
- Some code stays in the body only because of `import defer`: the call of a deferred module that gets no init, and the preparation of a deferred import of a module with top-level `await`. The build without `import defer` runs that module when the chunk loads its chunk. A declined init at or after the import of that chunk would run the other module first, in the opposite order of that build. Thus the chunk adds only the declined inits before that import.
- The modules of the body before the importer move to a new common chunk, when the move keeps the order of the other imports. The importer is then the first module, so the output can express `x`, `a`, `y` for `import './x.js'; import './a.js'; import './y.js'`. The cost is one more chunk, only in a build with `import defer`, and only when a later import of the importer runs code at load.
- A common host gives up the modules that run at load and come after its first deferred module in execution order. They move to a new common chunk, and the runtime module and the deferred modules stay. Else the host runs such a module `w` at load, before the init of `a`, and `import './a.js'; import './w.js'` runs `w` first. The cost is one more chunk, only in a build with `import defer`, and only when a common chunk holds a deferred module with an eager importer in another chunk and a later module that runs at load.
- Shared manual groups can have mutually incompatible entry orders. Synchronous siblings can stay together behind separate initializers. If a group still needs a global init that would run in the wrong entry, Rolldown warns and preserves independent evaluation boundaries for its movable eager members.
- When a common host cannot move its later modules, it runs its deferred modules at load, at their places, as the build without `import defer` does. For example, the later module has top-level `await`, the host is the chunk of a code splitting group, or a second deferred module comes between two later modules. Else the host runs the later modules at load, before the init, and the order is worse than the order of that build. The cost: a deferred-only importer that loads the host also runs these modules at load, as in that build.
- The host does not import the chunk of the importer. In such a cycle, the module can read bindings of the chunk before its body runs.
- With `strictExecutionOrder`, a common chunk that holds an entry module is not a host. The order wrappers move such an entry into a common chunk that does not run it at load, also without `import defer`. That gap belongs to the order wrappers, so this design keeps the order of the build without `import defer` there.

### Rejected alternatives

- A host that runs the module at load whenever the module has an eager importer. The order is then correct, but a deferred-only importer that loads the host also runs the module. This trades an order bug for a deferral bug. Thus a host runs a module at load only when it cannot move its later modules.
- A whole-build fallback for a common host that cannot move its later modules. The decision runs before the link stage, and the chunks do not exist then. Most reasons depend on the chunks, for example a dynamic import from one part of the host to the other.
- A new chunk between each two deferred modules of a common host. Each part needs the checks of the split, and the output gets more chunks.
- Keep the call in the body when the plan declines an importer. Then code of a later import runs before the module, also where the build without `import defer` runs the module first. For example, in `cjs` output, `import x from './x.cjs'; import './a.js'; import './y.js'` runs `y` before `a`, and `y` can read a side effect of `a` that is not there yet.
- A fallback for each build where the plan can decline an importer. The decision runs before the link stage, and the plan needs the chunks. A decision after the link stage cannot give the output of the build with the option off, because the link stage of that build gives other results, for example the wrappers.
- Move the deferred modules out of a common host, instead of the modules that run at load. The runtime module is often in that chunk, and the deferred modules need its helpers, so the move must also move or share the runtime helpers.
- Keep the init right after the import of the host, and stop the plan at a later deferred module of the host after an import of another chunk. With the split of the host, `import './a1.js'; import './w.js'; import './a2.js'; import './y.js'` then runs `y` before `a2`. Without the split, the output ran `a2` before `y`.
- An init before the body of the chunk for any importer, with no first-module rule. It runs `a` before `y`, but also before an earlier module `x` of the body: native ESM runs `x` first.
- A call of `init_x()` in the file of the next module by execution order, across chunks. The order in the bundle is correct, but a direct import of that file also runs the deferred module. Native ESM runs only the imported module.
- An import of the init chunk in each chunk with a static importer of the host. A chunk with a deferred-only importer then runs the module at load. In a cycle, the module runs before the bindings of the chunk exist. This form failed in 7 of 7 tested trees.
- An init file that the bundler adds after the chunks render. `renderChunk` and `augmentChunkHash` do not see the file, and the `imports` of the importer in `renderChunk` do not list it. A plugin asset with the same name gave two outputs with one name, with no warning.

## `import.defer()`

### Goal

`import.defer('./x.js')` gives the deferred namespace object of `x.js` after the load of its chunk, and evaluates nothing. Each call and each static deferred import of `x.js` give the same object.

### Decisions

- A call reads `x_deferred`, the object of the static form. Thus both forms give one object, and they share the closure wrappers, the state records and the preparation.
- An async arrow function holds the read: `(async () => x_deferred)()`. Thus a failed preparation rejects the promise, and the call does not throw synchronously, as for `import()`. `x_deferred` has no `then` key, so the resolution of the promise does not evaluate the module.
- In the same chunk, a call can run before the code of its target. Thus the declarations of the target and of its closure move to the start of the chunk. The code of a moved module has only declarations, so the move changes no evaluation order.
- A module that its common chunk runs at load keeps its init call at its old place, after the code of the last module before it that does not move. The build without `import defer` runs the module at that place.
- A build falls back when an `import.defer()` call loads a CommonJS module. Native ESM defers the evaluation of a CommonJS module also for this form. The deferred namespace of a CommonJS module belongs to a static import record (`import_x`, `x_deferred` of `create_deferred_cjs_namespaces`), and an `import()` of the module evaluates it when the chunk loads. The fallback gives the order of the build without `import defer`, with a warning.
- With `codeSplitting: false`, an `import()` makes a module with no module syntax CommonJS, as in the build without `import defer`. The fallback check uses the same rule, so such a target also makes the build fall back.
- A call with a `/* @vite-ignore */` comment becomes `import()`, with a warning, as a call with a specifier that is not static. The scanner adds no import record for such a call, so the build cannot defer its module. Before this step, each call got such a warning, so the comment must not remove it.

### Rejected alternatives

- A lowering of only the `import.defer()` calls of CommonJS modules to `import()`. The module then evaluates when its chunk loads, also before a static deferred access to the same module, and the build has no warning for it. The other conditions also decide for the whole build (see "Whole-build fallback").
- A deferred namespace of a CommonJS module for `import.defer()`. The chunk of the module must then export an object that calls `require_x()` at the first access, outside the records of the importers. This needs a new symbol for each CommonJS target. A later step can add it.
- No move for a module that its chunk runs at load. A read of its namespace object during a pause at a top-level `await` then gets `undefined`, because the code of the module did not run yet.
- An await of the end of the chunk before the call reads `x_deferred`. A call at the top level of the chunk, with `await`, then waits for itself (fixture `dynamic_same_chunk_top_level_await`).

## External modules

### Goal

A deferred import of an external module, and an import of an external module in a module of a closure, keep the native order where the output format can delay the external module. Where the format cannot delay it, the build gives the order of the build with the option off, with a warning. The order must not be worse than the order of the build without `import defer`.

### Decisions

- `cjs` output loads an external module with `require()` at the place of the import, in the init of the module of the closure. A deferred import loads the external module at the first access. `require()` is synchronous, so a deferred namespace can call it at the first access.
- `esm` output keeps `import defer` and `import.defer()` of an external module, and the runtime defers the module. An eager import of an external module in a module of a closure makes the build fall back. The chunk imports the external module at its start, so the module evaluates when the chunk loads.
- `iife` and `umd` output get an external module as a global or as a wrapper argument, so the module evaluates before the bundle runs. The build falls back for each import of an external module in a closure, and for each deferred import of an external module.
- A builtin module and an external module with no side effects do not make a build fall back. Their early evaluation is not observable.
- In `cjs` output with `dynamicImportInCjs: true`, `import.defer('ext')` keeps `import()`, with a warning. `require()` throws for an ES module with top-level `await` (`ERR_REQUIRE_ASYNC_MODULE`), but `import()` loads it. The build with the option off keeps `import()` too, so `require()` would make a working build fail. With `dynamicImportInCjs: false`, the build with the option off also loads the module with `require()`, so the call gets a deferred namespace.
- In `cjs` output, the build falls back when the `export *` chain of a module of a closure reaches an external module, and the module can be the entry module of an output file. The file copies the keys of the external module to its exports when it loads. Thus the external module evaluates at load, and the module of the closure at the first access. Neither native Node nor the build with the option off gives this order.
- The checks run before the link stage, with the other conditions of the fallback (see "Whole-build fallback"). They do not know the chunks. Thus an external module that an eager importer outside the closures also imports still makes an `esm` build fall back.

### Rejected alternatives

- An eager external module with a warning, for `esm`, `iife` and `umd` output: the chunk imports the external module at its start, and the build keeps the deferral of the other modules. The order is then neither the native order nor the order of the build with the option off. Example: `main.js` has `import defer * as a from './a.js'; globalThis.counter++` and reads `a.seen`. `a.js` has `import 'ext'; export const seen = globalThis.counter`. `ext` sets `counter = 0`. Native Node gives 0 (main, ext, a). The build with the option off gives 0 (ext, a, main). An eager external module gives 1 (ext, main, a).
- `require()` for `import.defer('ext')` in `cjs` output with `dynamicImportInCjs: true`. See the decisions.
- A lazy copy of the keys for the `export *` of an entry module. An importer of the file reads the keys when the file loads, and the keys need the evaluated module.

### RFC parts marked unreasonable

The implementation follows the `import defer` RFC for external modules, except in the parts below. Each part is marked **Unreasonable**, with a quote from the RFC, the reason, what the implementation does, and the tests. A review against the RFC can start here.

- **Unreasonable: an eager external module with a warning.** The RFC table "Output formats and externals" has the row "An external module that a module of a deferred closure imports eagerly", with "Eager. Warning `DEFERRED_EXTERNAL_EVALUATED_EAGERLY`." for ESM output and "Eager. Warning." for IIFE and UMD output. That order is neither the native order nor the order of the build with the option off (see the example in "Rejected alternatives"). Instead, the build falls back, and the warning names the external module. The warning `DEFERRED_EXTERNAL_EVALUATED_EAGERLY` and the key `checks.deferredExternalEvaluatedEagerly` do not exist. Tests: `transitive_external_fallback`, `external_iife_umd` and the Node test `packages/rolldown/tests/import-defer-fallback.test.ts`.
- **Unreasonable: `require()` for `import.defer()` of an external module with `dynamicImportInCjs: true`.** The RFC table gives `(async () => __deferNsCached("ext", () => __toESMCached("ext", require("ext"))))()` for CommonJS output. Its known limits say: "External modules in CommonJS output must load with `require()`. An ES module with top-level await cannot load that way". The build with the option off loads such a module with `import()`. Instead, the call keeps `import()`, with a warning, and gets the RFC form only with `dynamicImportInCjs: false`. Test: `external_cjs_dynamic_import`.
- **Unreasonable: no rule for an `export *` of an external module in `cjs` output.** For an external module that a module of a deferred closure imports eagerly, the RFC table gives "A lazy `require("ext")` in the init wrapper of the importer" for CommonJS output. But the output file of an entry module also loads the external modules of its `export *` chain when the file loads. Instead, the build falls back (see the decisions). Tests: the two `export *` cases of the Node test `packages/rolldown/tests/import-defer-fallback.test.ts`.

## Deferred groups

### Goal

Each lazy init wrapper has a cost in the engine at startup (the parse of the function) and at the first use (the compile and the call). A deferred closure can have hundreds of modules, and without groups each module has its own wrapper. A group gives one wrapper for each root, so that the engine parses and compiles fewer functions.

### Measurements

Bench: a 1.4 MB bundle of open-source packages (graphql, the AWS SDK, zod, React, the AI SDK), with four deferred routes, minified, in one chunk. Node 26, 60 rounds in fresh processes, medians, against the same build without groups:

| Build                                          | Init wrappers | Startup  | First use of all routes |
| ---------------------------------------------- | ------------- | -------- | ----------------------- |
| Without groups                                 | 546           | 20.40 ms | 19.76 ms                |
| Groups, a shared module has no group           | 118           | −0.40 ms | −2.14 ms                |
| Groups, a shared module can have its own group | 33            | −0.62 ms | −2.38 ms                |

The size goes from 1,422,419 B to 1,394,787 B.

The CommonJS wrappers of the groups (11 in this bench) then move into the root wrappers. Two more runs of 60 rounds compare this step against the groups without it. The second run measures the code after the fixes of the `Init` steps and of the split. In the same run, the fixes have no measurable effect against the build before them: startup 0.00 ms, first use of all routes −0.14 ms.

| Run    | Startup  | First use of all routes | First use of the route with React server rendering | webpack, same route |
| ------ | -------- | ----------------------- | -------------------------------------------------- | ------------------- |
| First  | −0.28 ms | +0.72 ms                | 0.52 → 1.05 ms                                     | 3.31 ms             |
| Second | −0.46 ms | +0.74 ms                | 0.55 → 1.12 ms                                     | 3.48 ms             |

The bench has one chunk, so no chunk runs a deferred module at load (see the decisions).

### Decisions

- Only plain `import` edges make a member. A `require()` target can run conditionally. `ImportKind::is_static()` also includes `require`, `new URL` and the CSS kinds, so the closure of a deferred import can contain modules that the root must not run.
- A member has callers only in its own group, and it is in the same chunk as its root. Thus only the wrapper of the root can start its body, and the error and the order rules of one wrapper stay correct: a throw in a member ends the init of the root, and a second call throws the same error again.
- A module that other code can call keeps its wrapper (see "Membership" in implementation.md). A module that the closures of two roots contain keeps its wrapper, and it can have its own group.
- A module whose wrapper a chunk exports keeps its wrapper, also when no code calls it. The cross-chunk links add the wrapper of the owner of each imported symbol, before the groups exist. This costs one wrapper for each such module, and only with code splitting.
- The member bodies run in the depth-first order of the steps, and a pre-call keeps a module outside the group before a member body. The alternative was to keep the init calls of the members as they are, which needs a wrapper for each member.
- The declarations of the members stay at the top level of the chunk. Code outside the group can read them (namespace getters, `(init_r(), x)` reads, exports to other chunks).
- A statement that writes a binding of an import (a CommonJS import, a deferred CommonJS namespace, the `__reExport` of an `export *`) moves before the next member body. A member body runs before the whole body of its importer, but without groups this statement runs before the init calls of the later imports. The statement moves one time and as a whole, with its names in the hoisted `var` list. Thus the order of evaluation and the identity of each namespace object are the same as without groups and in native ESM.
- The finalizer records where the body of a root or a member starts, as an index in its program. The two parts are printed on their own, and the wrapper text of the root comes from a print of the wrapper with an empty closure. Thus no part of the text that the renderer splits comes from user code.
- A CommonJS wrapper that only one group uses goes into the root wrapper. A factory at the top level has the eager-compile hint (`pife`), so the engine compiles it at startup. In the root wrapper, the engine compiles it with the root wrapper at the first use. This moves compile time from startup to the first use: the startup is the cost that `import defer` must make smaller.
- No groups in strict execution order, dev mode, `profilerNames` and inline common chunks. These paths put their own calls or names on each wrapper.
- No group for a deferred module that its common chunk runs at load (see "Run at load" in implementation.md), or for an anchor: the module whose code ends with the init call of such a module (see "Declarations before the first call" in implementation.md). The finalizer puts this call at the top level of the program. In a group module, that place is the body, which runs in the wrapper of the root. Thus the call would not run at load, and a later module of the chunk would run first (fixture `group_run_at_load_anchor`).

### Rejected alternatives

- One lazy function for each root that contains all declarations and bodies of its closure: in a model build, the first use was 1.58 ms slower.
- No group for a module in a cycle that reads a binding of an import before the statement of the binding runs. A cycle is not necessary for this failure: a member can change the exports of a CommonJS module before the root makes its namespace (fixture `group_cjs_exports_snapshot`). A cycle check thus does not find all cases.
- A marker statement in the printed code, and a split of the text at the line of the marker. User code can contain the same text, also on a line of its own in a template literal (fixture `group_marker_text`).
- Remove the export of a member wrapper and its import in the other chunk. This needs proof that no code of the importing chunk calls the wrapper. When the wrapper stays, the output stays valid without that proof (fixture `group_exported_wrapper`).

## PIFE hint of deferred wrappers

### Goal

`optimization.pifeForModuleWrappers` puts the closure of a module wrapper in parentheses. This hint makes the engine compile the closure eagerly (<https://v8.dev/blog/preparser#pife>). The option is off by default for `platform: 'neutral'`. For the wrapper of a deferred module, the hint has no cost at load time, so the build keeps it.

### Decisions

- The wrapper of a module that `import defer` wraps keeps the hint also for `platform: 'neutral'`. The wrapper is in its hoisted `function init_x()`, and the engine compiles `init_x` only at the first access. The closure runs right after that, so the hint compiles nothing at load time.
- An explicit `pifeForModuleWrappers: false` turns off the hint also for these wrappers.
- A root of a group gets the text of its wrapper from a print of the wrapper with an empty closure. Thus the root has the same hint as a wrapper with no group (fixture `group_pife`).

## Unresolved questions

- Which steps remove which conditions: `experimentalInlineCommonChunks`, `includeDependenciesRecursively: false`, the CommonJS files that run their module at load, and the `import.defer()` calls of CommonJS modules.
- Can the check of `esm` output skip an external module that an eager importer outside the closures, in the same chunk, also imports? The check runs before the chunks exist.
- Can the file of an entry get a facade with `preserveModules`, as without `preserveModules`? A deferred importer of the entry module in another file then does not run the module at load. With `strictExecutionOrder`, the order wrappers already move the module out of the file of the entry.
- Can `esm` output keep `import.defer()` for a call that the build does not process: a call with a `/* @vite-ignore */` comment, or with a specifier that is not static? The runtime then defers the module, but it must support `import.defer()`.
- Can the preparation run at the position of the deferred import in a module with no wrapper? It must run before the code of the first later import, at the top level of the chunk, as `ensure_lazy_module_initialization_order` moves an init call (`Chunk::insert_map`). Then the order rule can go away.
- Can a group move the private declarations of its members into the wrapper of the root? In a model build, the saving was near the noise floor.
- Can a CommonJS wrapper that two groups share leave the top level? There it keeps its eager-compile hint (`pife`), which costs startup time. The bench of "Deferred groups" has no such wrapper.

## Related

- [implementation.md](./implementation.md) — the scan, the fallback, the link stage, the finalizer, the CommonJS targets, the lazy hosts, `import.defer()`, the external modules and the deferred groups
