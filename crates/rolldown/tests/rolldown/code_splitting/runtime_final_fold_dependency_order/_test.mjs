import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
assert.equal((await import('./dist/a.js')).value, 'ok');
assert.ok(!existsSync(new URL('./dist/rolldown-runtime.js', import.meta.url)));
