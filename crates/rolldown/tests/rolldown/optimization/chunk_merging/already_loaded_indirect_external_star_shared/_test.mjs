import assert from 'node:assert';

const { app } = await import('./dist/main.js');
const appNs = await app;

// The barrel is a chunk of its own here, so its namespace object crosses the chunk
// boundary before `__reExport` merges it into the extracted namespace.
assert.deepEqual(Object.keys(appNs).sort(), ['done', 'marker', 'own']);
assert.strictEqual(appNs.marker, 'from-ext-shared');
assert.strictEqual(appNs.own, 43);
assert.strictEqual(await appNs.done, 84);

const { otherMarker } = await import('./dist/other.js');
assert.strictEqual(otherMarker, 'from-ext-shared');
