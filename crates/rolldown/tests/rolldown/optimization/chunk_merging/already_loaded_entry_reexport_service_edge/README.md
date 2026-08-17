# Already-loaded fold vs. entry re-export service edge

The entry re-exports `q` from a pure module without using it, and a dynamic
consumer both uses `q` and runs a side-effectful module that reads `a` from a
shared pure module.

## Graph

- `entry.js` uses `helper` from `m.js`, which reads `a` from `x.js`.
- `entry.js` re-exports `q` from `w.js` and never uses it.
- `entry.js` dynamically imports `d.js`.
- `d.js` uses `q` from `w.js` and imports side-effectful `y.js`, which reads
  `a` from `x.js` at its top level.

## The bug this pins

`x` is statically loaded by the entry, so the already-loaded pass folds it into
the entry chunk. The fold's cycle check predicts static imports from
`load_dependencies`, which does not carry the entry's unused re-export of `q`.
Emission does: the entry chunk must import `q` from the chunk that owns `w` to
serve the re-export. The emitted graph is `entry -> d -> entry`, and `d`'s
top-level read of `a` observes an uninitialized binding.

While the bug exists, `expectExecutionFailure` pins the runtime crash. The fix
must reject the fold, keeping the chunk graph acyclic.
