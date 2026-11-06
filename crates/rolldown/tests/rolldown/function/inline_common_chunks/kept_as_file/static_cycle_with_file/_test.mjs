import assert from 'node:assert';

// `r.js` reads `g` while `g.js` is still initializing (the cycle is entered from `g`), so
// `r` is `'r' + undefined`; the value is what the same build with the option off produces.
globalThis.events = [];
await import('./dist/a.js');
await import('./dist/b.js');

assert.deepStrictEqual(globalThis.events, ['A rundefinedrundefined', 'B rundefined']);
