import assert from 'node:assert';
import fs from 'node:fs';

// The collapse carries the entry trigger, so nothing has to stay at the specifier the source wrote.
assert.strictEqual(fs.existsSync(new URL('./dist/target.js', import.meta.url)), false);

const { targetPromise } = await import('./dist/a.js');
const target = await targetPromise;

assert.strictEqual(target.value, 1);
// `observer.js` shares the host chunk but stays behind its own wrapper, so loading the host
// for the target must not run it.
assert.deepStrictEqual(globalThis.events, ['target']);
