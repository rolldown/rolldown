import assert from 'node:assert/strict';
import { copyFileSync } from 'node:fs';

copyFileSync(new URL('./pause.mjs', import.meta.url), new URL('./dist/pause.mjs', import.meta.url));
await import('./dist/main.js');
assert.deepEqual(globalThis.calls, ['a done']);
