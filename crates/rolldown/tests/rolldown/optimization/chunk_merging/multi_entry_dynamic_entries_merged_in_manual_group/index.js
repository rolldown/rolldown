import assert from 'node:assert';

// `index` dynamically imports all three of a/b/c. With two static entries and a
// `shared-abc` manual group merging the targets, each `import('./x.js')` must be
// rewritten to load the merged chunk directly (no per-target proxy chunk).
const [a, b, c] = await Promise.all([import('./a.js'), import('./b.js'), import('./c.js')]);

assert.strictEqual(`${a.A}${b.B}${c.C}`, 'ABC');
