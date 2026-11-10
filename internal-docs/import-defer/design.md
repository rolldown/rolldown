# Import defer — Design

> How this is realized in code lives in [implementation.md](./implementation.md).

This doc records only the decisions about the deferred groups.

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

### Open questions

- Move the private declarations into the group wrapper. In a model build, the saving was near the noise floor.
- A CommonJS wrapper that two groups share stays at the top level, with its eager-compile hint (`pife`), which costs startup time. The bench has no such wrapper.

## Related

- [implementation.md](./implementation.md) — the deferred groups in code
