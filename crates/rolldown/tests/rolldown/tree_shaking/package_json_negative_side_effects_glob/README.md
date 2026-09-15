# Leading-bang package side-effect glob

A package's `sideEffects` array is a positive allowlist, so a leading-`!` entry is not a negation — it is a literal path, which `!foo.js` legally is on POSIX. Rolldown escapes the `!` before handing the pattern to `fast_glob`, which would otherwise read it as a negation and mark every path that does not match the rest of the pattern as side-effectful.

The package only marks `*.effect.js` files as side-effectful. Both effect files remain, while the unused ordinary module is removed even though it contains a top-level effect.
