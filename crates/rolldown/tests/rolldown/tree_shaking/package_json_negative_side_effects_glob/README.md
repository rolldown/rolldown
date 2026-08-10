# Leading-bang package side-effect glob

A package's `sideEffects` array is a positive allowlist. Vite 6 does not treat a leading-`!` entry as a positive match. Rolldown must not pass it directly to `fast_glob`, where it would mark every path that does not match the rest of the pattern as side-effectful.

The package only marks `*.effect.js` files as side-effectful. Both effect files remain, while the unused ordinary module is removed even though it contains a top-level effect. This matches Vite 6 package-side-effect handling.
