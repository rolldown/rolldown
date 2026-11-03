# Already-loaded fold vs. entry re-export service edge

The entry re-exports `q` from a pure module without using it, and a dynamic
consumer both uses `q` and runs a side-effectful module that reads `a` from a
shared pure module.

## Graph

- `entry.js` reads `a` from `x.js`.
- `entry.js` re-exports `q` from `w.js` and never uses it.
- `entry.js` dynamically imports `d.js`.
- `d.js` uses `q` from `w.js` and imports side-effectful `y.js`, which reads
  `a` from `x.js` at its top level.

## The bug this pins

`x` is statically loaded by the entry, so the already-loaded pass wants to fold
it into the entry chunk. The fold's cycle check predicts static imports, and a
prediction built only from `load_dependencies` misses the entry's unused
re-export of `q`. Emission serves that re-export with a real import: the entry
chunk imports `q` from the chunk that owns `w`. With the missed edge the fold
was accepted and the emitted graph was `entry -> d -> entry`, so `d`'s
top-level read of `a` observed an uninitialized binding.

The prediction now derives entry-export service targets from the entry's live
`resolved_exports`, the cycle check sees `entry -> d`, and the fold is
rejected. `x` stays in its own shared chunk and the graph is acyclic.
