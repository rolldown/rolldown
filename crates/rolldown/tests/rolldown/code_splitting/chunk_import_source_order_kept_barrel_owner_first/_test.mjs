import assert from 'node:assert/strict';

// `main.js` gets `SIZE` through the barrel `ui/index.js`, which has side effects. `ui/button.js`
// declares `SIZE` and has no side effects, so the chunk of the barrel does not import it, but
// `ui/button.css.js` must still run before the body of the barrel, as in the source. So the chunk
// of `ui/button.js` keeps its place before the chunk of the barrel.
await import('./dist/main.js');
assert.deepEqual(globalThis.loadOrder, ['style', 'barrel', 'main md']);
