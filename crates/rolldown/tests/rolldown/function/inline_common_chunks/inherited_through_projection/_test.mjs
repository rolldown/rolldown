import assert from 'node:assert';

// `a` reads the records `r` and `s`; `r` imports the file `v`, which reads `s`. After projection
// `a` imports `v`, whose registration of `s` runs before `a`'s body, so `a` prints only `r`'s
// factory.
globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');
await import('./dist/c.js');

assert.deepStrictEqual(globalThis.events, ['S body', 'A 1 2', 'B 3', 'C 4']);
