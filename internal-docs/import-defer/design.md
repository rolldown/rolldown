# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

This doc records the decisions about the deferred groups, about the shared namespaces of a CommonJS defer target, and about the init after the import of a lazy host.

## Deferred groups

### Goal

Each lazy init wrapper has a cost in the engine at startup (the parse of the function) and at the first use (the compile and the call). A deferred closure can have hundreds of modules, and each module had its own wrapper. A group gives one wrapper for each root, so that the engine parses and compiles fewer functions.

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

### Rejected alternatives

- One lazy function for each root that contains all declarations and bodies of its closure: in a model build, the first use was 1.58 ms slower.
- No group for a module in a cycle that reads a binding of an import before the statement of the binding runs. A cycle is not necessary for this failure: a member can change the exports of a CommonJS module before the root makes its namespace (fixture `group_cjs_exports_snapshot`). A cycle check thus does not find all cases.
- A marker statement in the printed code, and a split of the text at the line of the marker. User code can contain the same text, also on a line of its own in a template literal (fixture `group_marker_text`).
- Remove the export of a member wrapper and its import in the other chunk. This needs proof that no code of the importing chunk calls the wrapper. When the wrapper stays, the output stays valid without that proof (fixture `group_exported_wrapper`).

### Open questions

- Move the private declarations into the group wrapper. In a model build, the saving was near the noise floor.
- A CommonJS wrapper that two groups share stays at the top level, with its eager-compile hint (`pife`), which costs startup time. The bench has no such wrapper.

## Shared CommonJS namespaces

### Goal

The proposal gives one deferred namespace for each module. Before this step, each `import defer` of a CommonJS module had its own `__deferNs` object and its own `__toESM` namespace, also for two imports in one file. An eager import shares one namespace for the importers with no wrapper in one chunk (`merge_cjs_namespace`). The deferred form now has the same scope.

### Decisions

- The deferred records have their own groups, apart from the groups of `merge_cjs_namespace`. An eager import assigns `import_x` at the import statement, and a deferred access assigns it at the first access. One shared binding would get two `__toESM` objects in one run.
- The interop mode is part of the key. The eager merge keys by module only, so its merged binding has the mode of the first importer. A deferred access assigns `import_x` with the mode of its own importer, so a merge by module only would make the mode depend on the access that runs first.
- Only importers with no wrapper take part, as in the eager merge. A wrapped importer declares its symbols inside its wrapper, where other modules cannot read them.
- Nothing merges with `strictExecutionOrder`, as in the eager merge. The order wraps of the generate stage give the importers wrappers that `wrap_kind()` does not show. A merge then puts the declaration in the wrapper of one importer, and another importer can read it before that wrapper runs. The strict variant of the fixture `commonjs_shared_namespace` checks this rule.

### Rejected alternatives

- A declaration at the top of the chunk, which also covers the importers with a wrapper. `__deferNs` evaluates nothing, so the declaration can move there. This needs new placement code, and the eager namespace has the same gap.
- One object in the chunk of the target, which also gives one object across chunks. This needs more work, and the eager namespace has the same gap across chunks.
- The first symbol as the root, and a new key in `exports_to_other_chunks` after the merge. The `esm` output reads the old keys through `imports_from_other_chunks`, so the import lists must change too, and two keys can become one. A root that is a key of the table keeps every key valid with no change to the tables.

## Init after the import of a lazy host

### Goal

A lazy host is the entry chunk of a deferred ES module, and its load does not run the module. An eager importer in another chunk calls `init_x()` in its body, after all imports of its chunk. Thus `import './a.js'; import './y.js'` runs `y` before `a`, and native ESM runs `a` first. With `preserveModules`, each deferred ES module has a lazy host, so this order occurs in most such builds. A common chunk that holds a deferred module also does not run it at load, so the same order occurs for it. The goal is the native order for each importer where a move of the init keeps the order of all other steps.

### Decisions

- The chunk of the importer runs the init right after its import of the host. In `esm` output, it imports an init chunk with no modules, and the init chunk calls `init_x()`. In `cjs` output, it calls `require_x.init_x()` after the `require()` of the host. The load of the host still runs nothing, so a deferred-only importer still runs no module at load.
- When the import of an earlier module of the importer comes after the host, the init follows that import. In `import './a1.js'; import './w.js'; import './a2.js'`, with `a1` and `a2` in one common host and `w` in another chunk, the init of `a2` follows the import of `w`. An init right after the host runs `a2` before `w`.
- The host is the lazy host or a common chunk. An init chunk calls the init function of one module. A common chunk can hold more than one deferred module, and the init functions of the other modules must stay lazy.
- The init chunk is a chunk of the chunk graph, which the plan adds before the chunks get their names. Thus plugins see it in `renderChunk` and `augmentChunkHash`, the `imports` of each importer list it, and the hashes and the minifier apply to it. Its name does not take the name of an emitted asset.
- The plan takes an importer only when each step that native ESM runs before the module also runs before the init. An earlier import that runs code in the body of the chunk stops the plan for the later hosts. The rules over-approximate: an earlier import whose module already ran can also stop the plan. A missed move keeps the order of the output without the plan, but a wrong move runs a module too early, and another module can then read a value that is not set yet.
- Only the first module of the chunk body that runs code takes part. The init runs before the body, so it also runs before each earlier module of the body.
- The modules of the body before the importer move to a new common chunk, when the move keeps the order of the other imports. The importer is then the first module, so the output can express `x`, `a`, `y` for `import './x.js'; import './a.js'; import './y.js'`. The cost is one more chunk, only in a build with `import defer`, and only when a later import of the importer runs code at load.
- A common host gives up the modules that run at load and come after its first deferred module in execution order. They move to a new common chunk, and the runtime module and the deferred modules stay. Else the host runs such a module `w` at load, before the init of `a`, and `import './a.js'; import './w.js'` runs `w` first. The cost is one more chunk, only in a build with `import defer`, and only when a common chunk holds a deferred module with an eager importer in another chunk and a later module that runs at load.
- The host does not import the chunk of the importer. In such a cycle, the module can read bindings of the chunk before its body runs.
- With `strictExecutionOrder`, a common chunk that holds an entry module is not a host. The order wrappers move such an entry into a common chunk that does not run it at load, also without `import defer`. That gap belongs to the order wrappers, so this design keeps the order of the build without `import defer` there.

### Rejected alternatives

- A host that runs the module at load when the module has an eager importer. The order is then correct, but a deferred-only importer that loads the host also runs the module. This trades an order bug for a deferral bug.
- Move the deferred modules out of a common host, instead of the modules that run at load. The runtime module is often in that chunk, and the deferred modules need its helpers, so the move must also move or share the runtime helpers.
- Keep the init right after the import of the host, and stop the plan at a later deferred module of the host after an import of another chunk. With the split of the host, `import './a1.js'; import './w.js'; import './a2.js'; import './y.js'` then runs `y` before `a2`. Without the split, the output ran `a2` before `y`.
- An init before the body of the chunk for any importer, with no first-module rule. It runs `a` before `y`, but also before an earlier module `x` of the body: native ESM runs `x` first.
- A call of `init_x()` in the file of the next module by execution order, across chunks. The order in the bundle is correct, but a direct import of that file also runs the deferred module. Native ESM runs only the imported module.
- An import of the init chunk in each chunk with a static importer of the host. A chunk with a deferred-only importer then runs the module at load. In a cycle, the module runs before the bindings of the chunk exist. This form failed in 7 of 7 tested trees.
- An init file that the bundler adds after the chunks render. `renderChunk` and `augmentChunkHash` do not see the file, and the `imports` of the importer in `renderChunk` do not list it. A plugin asset with the same name gave two outputs with one name, with no warning.

## Related

- [implementation.md](./implementation.md) — the deferred groups, the shared CommonJS namespaces and the init after the import of a lazy host in code
