import assert from 'node:assert/strict';
import { copyFileSync, mkdirSync } from 'node:fs';

const shared = new URL('./dist/shared/', import.meta.url);
mkdirSync(shared, { recursive: true });
copyFileSync(new URL('./sidecar.mjs', import.meta.url), new URL('sidecar.mjs', shared));

const source = await import('./mod.mjs');
assert.equal(source.foo, true);
assert.equal((await import('./dist/entries/mod.mjs')).foo, source.foo);
