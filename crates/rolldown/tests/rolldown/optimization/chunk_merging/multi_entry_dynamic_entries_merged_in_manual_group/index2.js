import assert from 'node:assert';

// Second static entry, dynamically importing a/b only. Exercises the merged
// `shared-abc` chunk being shared across more than one entry.
const [a, b] = await Promise.all([import('./a.js'), import('./b.js')]);

assert.strictEqual(`${a.A}${b.B}`, 'AB');
