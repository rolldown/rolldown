import assert from 'node:assert/strict';

// `app.js` gets `IconA` through the barrel `index.js`, which has side effects, so the output imports
// the chunk of `index.js` and gets `IconA` from the chunk of `a.js`. `factory.js` reads `version`
// from the barrel, so the barrel, the icons and the factory are one import cycle. The source enters
// the cycle at `index.js`, so `factory.js` runs before the icons. If the chunk of `a.js` came first,
// the output would enter the cycle at `a.js` and run `b.js` before `factory.js`.
await import('./dist/app.js');
assert.deepEqual(globalThis.loadOrder, ['a', 'b']);
