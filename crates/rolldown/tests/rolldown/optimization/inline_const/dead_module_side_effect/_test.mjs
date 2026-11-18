import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';

const files = readdirSync(new URL('./dist/', import.meta.url)).filter((f) => f.endsWith('.js'));
assert.deepEqual(files.sort(), ['a.js', 'b.js']);
