# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

## Summary

`experimental.importDefer` keeps the deferred evaluation of `import defer` in the output. The support grows in steps. Each step supports more shapes of a build, and a build with another shape falls back to an eager import. A build with the option off must give the same output as before the option existed.

## Whole-build fallback

### Goal

A build with a shape that the deferral does not support yet gives the output of the build with the option off, and a warning for each deferred import. Thus each step can add the deferral for some shapes, and all other shapes keep the eager result that they had before.

### Decisions

- The decision is for the whole build, not for each import. The conditions (the output format, no external module in the closures, no CommonJS file that runs its module at load, the order rule of top-level `await`) are properties of the module graph and the options. A decision for each import needs rules for deferred and eager closures across chunks, cycles and async modules.
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
- A module with top-level `await` ends before the next import starts, as in a build without `import defer`. Thus the preparation waits for each module in the order of its walk. Native Node starts all of them first, and then waits. The fixture `tla_gather_order` checks the order of the walk.
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

## Unresolved questions

- Which steps remove which conditions: `import.defer()`, external modules, the other output formats, `experimentalInlineCommonChunks`, `includeDependenciesRecursively: false`, and the CommonJS files that run their module at load.
- Can the preparation run at the position of the deferred import in a module with no wrapper? It must run before the code of the first later import, at the top level of the chunk, as `ensure_lazy_module_initialization_order` moves an init call (`Chunk::insert_map`). Then the order rule can go away.

## Related

- [implementation.md](./implementation.md) — the scan, the fallback, the link stage, the finalizer, the CommonJS targets and the lazy hosts
