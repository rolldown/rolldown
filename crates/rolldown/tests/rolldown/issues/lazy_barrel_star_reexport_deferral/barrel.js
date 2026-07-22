// A "sideEffects": false barrel that re-exports its members only via "export *".
// lazyBarrel resolves the entry's named imports by probing these star targets in
// order, stopping once every requested name is found — so d.js (below the last
// needed name) is never loaded.
export * from './a.js';
export * from './b.js';
export * from './c.js';
export * from './d.js';
