import assert from 'node:assert';
import fs from 'node:fs';

// The call site now points at the host chunk, so the merged entry needs no file of its own.
assert.strictEqual(fs.existsSync(new URL('./dist/target.js', import.meta.url)), false);

const { targetPromise } = await import('./dist/a.js');
const target = await targetPromise;

assert.strictEqual(target.value, 1);
