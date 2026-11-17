import assert from 'node:assert';

// `target.js` re-exports a property of a CommonJS module. The property is read when
// `target.js` evaluates, so the later write in `page.js` must not change the value.
await import('./dist/main.js');
assert.deepStrictEqual(await globalThis.run(), { before: 7, value: 7 });
