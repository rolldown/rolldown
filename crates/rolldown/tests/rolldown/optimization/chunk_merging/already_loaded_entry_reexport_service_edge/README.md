# Lazy re-exports and initialized shared bindings

The entry re-exports `q` from a pure module without using it, and a dynamic
consumer both uses `q` and runs a side-effectful module that reads `a` from a
shared pure module.

## Graph

- `entry.js` reads `a` from `x.js`.
- `entry.js` re-exports `q` from `w.js` and never uses it.
- `entry.js` dynamically imports `d.js`.
- `d.js` uses `q` from `w.js` and imports side-effectful `y.js`, which reads
  `a` from `x.js` at its top level.

## Runtime requirements

Only the entry executes initially. Loading `d` evaluates `y` and then `d`,
and every read of `a` produces `value-a`. Loading `d` again does not repeat
their side effects. The shared binding has one definition.

These checks cover `false`, `exports-only`, `allow-extension`, and `strict`
entry signatures. They preserve lazy execution and detect reads of
uninitialized shared bindings.
