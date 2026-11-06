import assert from 'node:assert';

// `dep.js` has no module syntax, so Node loads it as CommonJS: its top-level `this` is its
// exports object, and `import()` returns that object as `default`. Without code splitting,
// `import()` is a `require()` that returns a promise, so `dep.js` is wrapped as CommonJS.
const { default: dep } = await import('./dep.js');
assert.strictEqual(dep.Thing, 'thing');
assert.strictEqual(globalThis.Thing, undefined);
