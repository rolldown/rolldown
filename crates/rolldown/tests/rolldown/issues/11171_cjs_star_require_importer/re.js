// The bundle moves the first import of `lib.cjs` to `mut.js`, which reaches `lib.cjs` earlier through `mid.cjs`.
// This default import keeps `export *` from being that import.
import lib from './lib.cjs';

export * from './lib.cjs';
import './mut.js';

export default lib;
